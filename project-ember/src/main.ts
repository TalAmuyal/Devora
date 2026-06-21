import '@xterm/xterm/css/xterm.css';
import { listen } from '@tauri-apps/api/event';
import { invoke, invokeLogOnly, Channel } from './invoke';
import { SessionManager } from './session/SessionManager';
import { TabBar } from './ui/TabBar';
import { OverlayManager } from './ui/OverlayManager';
import { KeyboardShortcuts } from './ui/KeyboardShortcuts';
import { CommandPalette } from './ui/CommandPalette';
import { showTextInputDialog } from './ui/components/TextInputDialog';
import { showAddRepoDialog } from './ui/components/AddRepoDialog';
import { showCloneRepoDialog } from './ui/components/CloneRepoDialog';
import { TaskCreationProgressHandle } from './ui/components/TaskCreationProgress';
import { WorkspaceHub } from './workspace/WorkspaceHub';
import { TaskCreationController } from './workspace/TaskCreationController';
import { CreationEvent, RepoInfo } from './workspace/types';
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

  // Resolve the per-profile terminal app command (e.g. nvim); undefined falls back to a plain shell.
  const resolveAppCommand = async (profilePath: string | null): Promise<string | undefined> => {
    if (!profilePath) return undefined;
    try {
      const defaultApp = await invokeLogOnly<string | null>('get_default_app', { profilePath });
      if (defaultApp && defaultApp !== 'shell') {
        return defaultApp;
      }
    } catch (_) {
      // fall back to plain shell
    }
    return undefined;
  };

  const openWorkspace = async (wsPath: string, title: string, repos: string[], profilePath?: string) => {
    dismissWsHub();

    const appCommand = await resolveAppCommand(profilePath ?? null);
    const cwd = repos.length === 1 ? `${wsPath}/${repos[0]}` : wsPath;
    try {
      await sessionManager.createSession(title, cwd, appCommand, wsPath, profilePath ?? null);
    } catch (e) {
      showError(`Failed to create session: ${e}`);
    }
  };

  const taskCreationController = new TaskCreationController({
    sessionManager,
    overlayManager,
    mainPanelEl,
    resolveAppCommand,
    onChange: () => tabBar.render(),
  });

  // Hand a new task off to the controller: close the Hub, then open a tab + progress overlay and run creation asynchronously so the window never freezes.
  const startTaskCreation = (
    taskName: string,
    repoPaths: string[],
    sourceWorkspacePath: string | null,
  ) => {
    const profilePath = wsHub.getActiveProfilePath();
    if (!profilePath) return;
    dismissWsHub();
    void taskCreationController.start(taskName, repoPaths, profilePath, sourceWorkspacePath);
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

  // Drive a two-phase creation dialog (add-repo, clone): swap in the progress view, run the backend `start` command, and translate its CreationEvent stream into progress updates.
  // Handles the cancel-before-the-backend-id race (cancel once the id arrives) and the failed-state footer (Esc/q closes instead of cancelling).
  // `onDone` (optional) receives the resulting workspace once it lands.
  const driveCreationDialog = (
    dialog: { showProgress(title: string): TaskCreationProgressHandle; close(): void },
    progressTitle: string,
    start: (onEvent: Channel<CreationEvent>) => Promise<number>,
    onDone?: (workspace: { path: string; name: string }) => void,
  ): void => {
    const progress = dialog.showProgress(progressTitle);
    let creationId: number | null = null;
    let cancelRequested = false;
    let failed = false;

    const requestCancel = () => {
      if (failed) {
        dialog.close();
        return;
      }
      if (creationId === null) {
        cancelRequested = true; // cancel once the backend id arrives
        return;
      }
      invokeLogOnly('cancel_workspace_creation', { id: creationId }).catch(() => {});
    };
    progress.onCancel(requestCancel);
    progress.onClose(() => dialog.close());

    const onEvent = new Channel<CreationEvent>();
    onEvent.onmessage = (event) => {
      switch (event.type) {
        case 'step':
          progress.setStep(event.label);
          break;
        case 'log':
          progress.appendLog(event.line);
          break;
        case 'done':
          dialog.close();
          onDone?.(event.workspace);
          break;
        case 'cancelled':
          dialog.close();
          break;
        case 'failed':
          failed = true;
          progress.showError(event.message);
          break;
      }
    };

    start(onEvent)
      .then((id) => {
        creationId = id;
        if (cancelRequested) requestCancel();
      })
      .catch(() => {
        // invoke already surfaced the error; the backend never started, so close the dialog.
        dialog.close();
      });
  };

  // Add a repo (git worktree) to the active session's workspace: pick a registered repo (+ optional postfix), then stream the worktree creation inside the dialog, with cancel support.
  const addRepoToWorkspace = async () => {
    // Capture the workspace up front so a mid-dialog tab switch cannot retarget the add.
    const session = sessionManager.getActiveSession();
    if (!session?.workspacePath || !session.profilePath) {
      showError('Cannot add a repo: the current session has no associated workspace');
      return;
    }
    const workspacePath = session.workspacePath;

    let repos: RepoInfo[];
    try {
      repos = await invoke<RepoInfo[]>('get_registered_repos', {
        profilePath: session.profilePath,
      });
    } catch (_) {
      return; // invoke already surfaced the error
    }
    if (repos.length === 0) {
      showError('No repos are registered in this profile');
      return;
    }

    const dialog = showAddRepoDialog({ repos });
    dialog.onSubmit(({ sourceRepoPath, worktreeDirName }) => {
      driveCreationDialog(dialog, `Adding: ${worktreeDirName}`, (onEvent) =>
        invoke<number>('add_repo_to_workspace', {
          workspacePath,
          sourceRepoPath,
          worktreeDirName,
          onEvent,
        }),
      );
    });
  };

  // Clone a git repo into a profile's repos/ dir from a pasted URL: prompt for the URL, then stream the clone inside the dialog, with cancel support.
  // `onDone` (optional) receives the cloned repo so callers can refresh their repo lists once it lands.
  const cloneRepoIntoProfile = (
    profilePath: string,
    onDone?: (repo: { path: string; name: string }) => void,
  ): void => {
    const dialog = showCloneRepoDialog({ profilePath });
    dialog.onSubmit(({ url }) => {
      driveCreationDialog(
        dialog,
        'Cloning repo',
        (onEvent) => invoke<number>('clone_repo_into_profile', { profilePath, url, onEvent }),
        onDone,
      );
    });
  };

  const wsHub = new WorkspaceHub(
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    startTaskCreation,
    dismissWsHub,
    (view) => openProfileManager(view),
    cloneRepoIntoProfile,
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
    onCloneRepo: (profilePath, onDone) => cloneRepoIntoProfile(profilePath, onDone),
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

  // Duplicate the active session into a new workspace: open the Hub's New Task form pre-filled with this session's repos (pinned to their current commits) and its title.
  const duplicateCurrentSession = () => {
    const session = sessionManager.getActiveSession();
    if (!session?.workspacePath || !session.profilePath) {
      showError('Cannot duplicate: the current session has no associated workspace');
      return;
    }
    wsHub.setActiveProfilePath(session.profilePath);
    wsHub.queueDuplication(session.workspacePath);
    openWsHub();
  };

  // Every palette command closes the palette (the active tab-covering overlay) before acting
  const closePaletteThen = (action: () => void) => () => {
    overlayManager.dismissTabCoveringOverlay();
    action();
  };

  const commandPalette = new CommandPalette({
    // Type-first palette: the search field is focused on open, so Escape closes the overlay directly (single press).
    onRequestClose: () => overlayManager.dismissTabCoveringOverlay(),
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
        id: 'add-repo',
        title: 'Add Repo to Workspace',
        description: 'Add a repo (git worktree) to the current workspace',
        icon: '⊕',
        shortcut: [],
        run: closePaletteThen(() => void addRepoToWorkspace()),
      },
      {
        id: 'clone-repo',
        title: 'Clone Repo into Profile',
        description: "Clone a git repo into the active profile's repos/ directory",
        icon: '⤓',
        shortcut: [],
        run: closePaletteThen(() => {
          const profilePath = wsHub.getActiveProfilePath();
          if (!profilePath) {
            showError('No active profile — open the Workspace Hub and set one first');
            return;
          }
          cloneRepoIntoProfile(profilePath);
        }),
      },
      {
        id: 'duplicate-session',
        title: 'Duplicate Current Session',
        description: 'Open the New Task form pre-filled to copy the current workspace',
        icon: '⧉',
        shortcut: [],
        run: closePaletteThen(() => duplicateCurrentSession()),
      },
      {
        id: 'close-session',
        title: 'Close Current Session',
        description: 'Close the active session tab',
        icon: '×',
        shortcut: [],
        run: closePaletteThen(() => {
          const id = sessionManager.getActiveSessionId();
          if (id !== null) sessionManager.closeSession(id);
        }),
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
    // Focus the search field after the overlay is attached (this overrides the wrapper focus showTabCoveringOverlay just set).
    commandPalette.focusSearch();
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
    // A creation overlay owns its own dismissal: Esc/q cancels the in-flight creation (or closes it after a failure).
    // The controller removes the overlay itself once that resolves.
    if (taskCreationController.isCreating(sessionId) && overlayManager.hasPanelOverlay(sessionId)) {
      taskCreationController.handleDismiss(sessionId);
      return;
    }

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
