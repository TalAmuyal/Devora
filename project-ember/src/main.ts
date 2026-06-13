import '@xterm/xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { invoke, invokeLogOnly } from './invoke';
import { SessionManager } from './session/SessionManager';
import { TabBar } from './ui/TabBar';
import { OverlayManager } from './ui/OverlayManager';
import { KeyboardShortcuts } from './ui/KeyboardShortcuts';
import { CommandPalette } from './ui/CommandPalette';
import { showTextInputDialog } from './ui/components/TextInputDialog';
import { WorkspaceHub } from './workspace/WorkspaceHub';
import { ProfileManager, ProfileManagerView } from './workspace/ProfileManager';
import { WebContentOverlay } from './webview/WebContentOverlay';
import {
  clearErrorBanners,
  installGlobalErrorHandlers,
  logToFile,
  scrapeErrors,
  showError,
} from './errors';

installGlobalErrorHandlers();

(window as any).__scrapeErrors = scrapeErrors;

document.addEventListener('DOMContentLoaded', async () => {
  // Surface Rust-side errors (logging::report_error) before anything else can emit one, so the window where app-error events are missed is minimal
  await listen<{ message: string }>('app-error', (event) => {
    showError(event.payload.message);
  });

  // Load theme early, before creating any terminals or UI, so CSS custom properties are set before first render
  try {
    const theme = await invokeLogOnly<Record<string, string>>('get_theme');
    const root = document.documentElement;
    for (const [prop, value] of Object.entries(theme)) {
      root.style.setProperty(prop, value);
    }
  } catch (_) {
    // fall back to CSS defaults
  }

  const appEl = document.getElementById('app')!;
  const mainPanelEl = document.getElementById('main-panel')!;
  const tabBarEl = document.getElementById('tab-bar')!;

  const overlayManager = new OverlayManager(appEl);
  const sessionManager = new SessionManager(mainPanelEl);

  const origActivate = sessionManager.activateSession.bind(sessionManager);
  sessionManager.activateSession = (id: number) => {
    origActivate(id);
    overlayManager.onSessionActivated(id);
  };

  const tabBar = new TabBar(tabBarEl, sessionManager, overlayManager);

  // The Workspace Hub and Command Palette are mutually-exclusive tab-covering overlays — neither opens while the other is open
  let commandPaletteOpen = false;

  const dismissWsHub = () => {
    overlayManager.dismissTabCoveringOverlay();
  };

  const openWorkspace = async (wsPath: string, title: string, repos: string[], profilePath?: string) => {
    dismissWsHub();

    let appCommand: string | undefined;
    if (profilePath) {
      try {
        const defaultApp = await invokeLogOnly<string | null>('get_default_app', { profilePath });
        if (defaultApp && defaultApp !== 'shell') {
          appCommand = defaultApp;
        }
      } catch (_) {
        // fall back to plain shell
      }
    }

    const cwd = repos.length === 1 ? `${wsPath}/${repos[0]}` : wsPath;
    try {
      await sessionManager.createSession(title, cwd, appCommand, wsPath, profilePath ?? null);
    } catch (e) {
      showError(`Failed to create session: ${e}`);
    }
  };

  // Swap the active session's task for a new one in the same workspace: validate the worktrees are idle, prompt for the new task name (pre-filled with the current one), write the new task identity, and rename the tab.
  const repurposeCurrentSession = async () => {
    // Capture the session up front so a mid-dialog tab switch cannot retarget the rename
    const session = sessionManager.getActiveSession();
    if (!session?.workspacePath) {
      showError('Cannot repurpose: the current session has no associated workspace');
      return;
    }
    try {
      const { currentTitle } = await invoke<{ currentTitle: string }>(
        'prepare_repurpose_task',
        { workspacePath: session.workspacePath },
      );
      const newTitle = await showTextInputDialog({
        title: 'Repurpose Workspace',
        initialValue: currentTitle,
        confirmLabel: 'Repurpose',
      });
      if (newTitle === null) return;
      await invoke('repurpose_task', { workspacePath: session.workspacePath, newTitle });
      session.setTitle(newTitle);
    } catch (_) {
      // invoke already surfaced the error
    }
  };

  const wsHub = new WorkspaceHub(
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    dismissWsHub,
    (view) => openProfileManager(view),
  );

  const teardownWsHub = () => {
    wsHub.unload();
  };

  const openWsHub = () => {
    wsHub.load();
    overlayManager.showTabCoveringOverlay(
      wsHub.getElement(),
      teardownWsHub,
      sessionManager.getActiveSession()?.terminalPane,
      undefined,
      // User dismissal (q/Esc/Ctrl+S) goes through the hub so it can close its cheatsheet first and refuse while zero-profile locked.
      () => wsHub.handleUserDismiss(),
    );
  };

  const toggleWsHub = () => {
    if (commandPaletteOpen) return;
    if (overlayManager.isTabCoveringOverlayActive()) {
      // Respect the active overlay's user-dismiss override (zero-profile lock, Profile-Manager-back-to-hub) instead of force-dismissing.
      overlayManager.dismissActiveOverlay();
    } else {
      openWsHub();
    }
  };

  const profileManager = new ProfileManager({
    getActiveProfilePath: () => wsHub.getActiveProfilePath(),
    setActiveProfilePath: (path) => wsHub.setActiveProfilePath(path),
    getOpenSessionsForProfile: (profilePath) =>
      sessionManager.getSessions().filter((s) => s.profilePath === profilePath),
    onClose: () => openWsHub(),
  });

  const openProfileManager = (view: ProfileManagerView = 'list') => {
    if (commandPaletteOpen) return;
    void profileManager.load(view);
    overlayManager.showTabCoveringOverlay(
      profileManager.getElement(),
      () => profileManager.unload(),
      null,
      undefined,
      // q/Esc/Ctrl+S on the Profile Manager returns to the Workspace Hub (showTabCoveringOverlay replaces this overlay and runs its cleanup).
      () => openWsHub(),
    );
  };

  // Every palette command closes the palette (the active tab-covering overlay) before acting
  const closePaletteThen = (action: () => void) => () => {
    overlayManager.dismissTabCoveringOverlay();
    action();
  };

  const commandPalette = new CommandPalette({
    commands: [
      {
        id: 'workspace-hub',
        title: 'Workspace Hub',
        description: 'List, filter and open workspaces',
        icon: '▦',
        shortcut: ['⌃', 'S'],
        run: closePaletteThen(openWsHub),
      },
      {
        id: 'new-shell',
        title: 'New Shell',
        description: 'Open a fresh shell tab',
        icon: '❯',
        shortcut: ['⌃', '⇧', 'S'],
        run: closePaletteThen(() => void sessionManager.createSession('Shell', undefined, undefined, null)),
      },
      {
        id: 'repurpose-session',
        title: 'Repurpose Current Session',
        description: 'Replace the finished task with a new one in the same workspace',
        icon: '↻',
        shortcut: [],
        run: closePaletteThen(() => void repurposeCurrentSession()),
      },
      {
        id: 'new-profile',
        title: 'New Profile',
        description: 'Create or register a profile root directory',
        icon: '＋',
        shortcut: [],
        run: closePaletteThen(() => openProfileManager('new')),
      },
      {
        id: 'manage-profiles',
        title: 'Manage Profiles',
        description: 'List, switch, create, and delete profiles',
        icon: '⚙',
        shortcut: [],
        run: closePaletteThen(() => openProfileManager('list')),
      },
    ],
    // One "Switch Profile: X" entry per non-active profile, re-resolved on every palette open so it tracks registrations and the active profile.
    dynamicCommands: async () => {
      const profiles = await invokeLogOnly<{ name: string; path: string }[]>('list_profiles');
      const activePath = wsHub.getActiveProfilePath();
      return profiles
        .filter((p) => p.path !== activePath)
        .map((p) => ({
          id: `switch-profile:${p.path}`,
          title: `Switch Profile: ${p.name}`,
          description: p.path,
          icon: '⇄',
          shortcut: [],
          run: closePaletteThen(() => {
            wsHub.setActiveProfilePath(p.path);
            openWsHub();
          }),
        }));
    },
  });

  const teardownPalette = () => {
    commandPalette.unload();
    commandPaletteOpen = false;
  };

  const openCommandPalette = () => {
    if (overlayManager.isTabCoveringOverlayActive()) return;
    commandPalette.load();
    overlayManager.showTabCoveringOverlay(
      commandPalette.getElement(),
      teardownPalette,
      sessionManager.getActiveSession()?.terminalPane,
      'overlay-passthrough',
    );
    commandPaletteOpen = true;
  };

  const openUserGuide = async () => {
    try {
      const guidePath = await invoke<string>('get_user_guide_path');
      const content = await WebContentOverlay.createMarkdownContent(guidePath, 'User Guide');
      overlayManager.showTabCoveringOverlay(
        content,
        undefined,
        sessionManager.getActiveSession()?.terminalPane,
        undefined,
        // Opened over the zero-profile welcome there is nothing behind the guide — return to the hub instead of an empty terminal area.
        () => {
          if (wsHub.isZeroProfileLocked()) {
            openWsHub();
          } else {
            overlayManager.dismissTabCoveringOverlay();
          }
        },
      );
    } catch (_) {
      // invoke already surfaced the error
    }
  };

  new KeyboardShortcuts(
    sessionManager,
    overlayManager,
    toggleWsHub,
    openCommandPalette,
    openUserGuide,
  );

  // Crit panel overlay integration: listen for backend events requesting
  // a Crit review overlay, and wire overlay dismissal back to the backend.
  await listen<{ ptyId: number; url: string }>('crit-open-overlay', (event) => {
    const { ptyId, url } = event.payload;

    const session = sessionManager.getSessions().find(s => s.getPtyId() === ptyId);
    if (!session) {
      logToFile('WARN', `crit-open-overlay: no session found for ptyId ${ptyId}`);
      return;
    }
    const content = WebContentOverlay.createUrlContent(url, 'Crit Review');
    overlayManager.showPanelOverlay(session.id, content, mainPanelEl, session.terminalPane);
    overlayManager.onSessionActivated(sessionManager.getActiveSessionId()!);
    tabBar.render();
  });

  const origDismissPanelOverlay = overlayManager.dismissPanelOverlay.bind(overlayManager);

  await listen<{ ptyId: number }>('crit-close-overlay', (event) => {
    const { ptyId } = event.payload;
    const session = sessionManager.getSessions().find(s => s.getPtyId() === ptyId);
    if (session) {
      origDismissPanelOverlay(session.id);
      tabBar.render();
    }
  });

  // Patch dismissPanelOverlay to notify the backend when a crit overlay is
  // dismissed by the user (q/Esc/tab close).
  overlayManager.dismissPanelOverlay = (sessionId: number) => {
    const hadOverlay = overlayManager.hasPanelOverlay(sessionId);

    origDismissPanelOverlay(sessionId);

    if (hadOverlay) {
      const session = sessionManager.getSessions().find(s => s.id === sessionId);
      const ptyId = session?.getPtyId();

      if (ptyId !== null && ptyId !== undefined) {
        invokeLogOnly('crit_overlay_dismissed', { ptyId }).catch(() => {});
      }

      tabBar.render();
    }
  };

  tabBar.render();

  openWsHub();

  (window as any).__test = {
    sessionManager,
    overlayManager,
    tabBar,
    wsHub,
    profileManager,
    openProfileManager,
    commandPalette,
    openCommandPalette,
    toggleWsHub,
    showError,
    clearErrorBanners,
  };
});
