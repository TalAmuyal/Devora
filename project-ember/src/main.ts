import '@xterm/xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { invoke, invokeLogOnly } from './invoke';
import { SessionManager } from './session/SessionManager';
import { TabBar } from './ui/TabBar';
import { OverlayManager } from './ui/OverlayManager';
import { KeyboardShortcuts } from './ui/KeyboardShortcuts';
import { CommandPalette } from './ui/CommandPalette';
import { WorkspaceHub } from './workspace/WorkspaceHub';
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
      await sessionManager.createSession(title, cwd, appCommand);
    } catch (e) {
      showError(`Failed to create session: ${e}`);
    }
  };

  const wsHub = new WorkspaceHub(
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    dismissWsHub,
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
    );
  };

  const toggleWsHub = () => {
    if (commandPaletteOpen) return;
    if (overlayManager.isTabCoveringOverlayActive()) {
      dismissWsHub();
    } else {
      openWsHub();
    }
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
        run: closePaletteThen(() => void sessionManager.createSession()),
      },
    ],
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
    commandPalette,
    openCommandPalette,
    toggleWsHub,
    showError,
    clearErrorBanners,
  };
});
