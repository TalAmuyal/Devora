import '@xterm/xterm/css/xterm.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { SessionManager } from './session/SessionManager';
import { TabBar } from './ui/TabBar';
import { OverlayManager } from './ui/OverlayManager';
import { KeyboardShortcuts } from './ui/KeyboardShortcuts';
import { WorkspaceHub } from './workspace/WorkspaceHub';
import { WebContentOverlay } from './webview/WebContentOverlay';

function logToFile(level: string, message: string): void {
  invoke('log_error', { level, message }).catch(() => {});
}

window.addEventListener('error', (e) => {
  logToFile('ERROR', `${e.message} at ${e.filename}:${e.lineno}:${e.colno}\n${e.error?.stack ?? ''}`);
});

window.addEventListener('unhandledrejection', (e) => {
  logToFile('ERROR', `Unhandled rejection: ${e.reason}`);
});

const origConsoleError = console.error.bind(console);
console.error = (...args: unknown[]) => {
  origConsoleError(...args);
  logToFile('ERROR', args.map(String).join(' '));
};

const origConsoleWarn = console.warn.bind(console);
console.warn = (...args: unknown[]) => {
  origConsoleWarn(...args);
  logToFile('WARN', args.map(String).join(' '));
};

document.addEventListener('DOMContentLoaded', async () => {
  // Load theme early, before creating any terminals or UI, so CSS custom
  // properties are set before first render.
  try {
    const theme = await invoke<Record<string, string>>('get_theme');
    const root = document.documentElement;
    for (const [prop, value] of Object.entries(theme)) {
      root.style.setProperty(prop, value);
    }
  } catch (e) {
    console.warn('Failed to load theme, using CSS defaults:', e);
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

  const dismissWsHub = () => {
    wsHub.unload();
    overlayManager.dismissTabCoveringOverlay();
    sessionManager.getActiveSession()?.terminalPane.focus();
  };

  const openWorkspace = async (wsPath: string, title: string, repos: string[], profilePath?: string) => {
    dismissWsHub();

    let appCommand: string | undefined;
    if (profilePath) {
      try {
        const defaultApp = await invoke<string | null>('get_default_app', { profilePath });
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
      logToFile('ERROR', `Failed to create session: ${e}`);
    }
  };

  const wsHub = new WorkspaceHub(
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    (path, title, repos) => openWorkspace(path, title, repos, wsHub.getActiveProfilePath()),
    dismissWsHub,
  );

  const toggleWsHub = () => {
    if (overlayManager.isTabCoveringOverlayActive()) {
      dismissWsHub();
    } else {
      wsHub.load();
      overlayManager.showTabCoveringOverlay(wsHub.getElement());
    }
  };

  const openUserGuide = async () => {
    try {
      const guidePath = await invoke<string>('get_user_guide_path');
      const content = await WebContentOverlay.createMarkdownContent(guidePath, 'User Guide');
      overlayManager.showTabCoveringOverlay(content);
    } catch (e) {
      console.error('Failed to open User Guide:', e);
    }
  };

  new KeyboardShortcuts(sessionManager, overlayManager, toggleWsHub, openUserGuide);

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
    overlayManager.showPanelOverlay(session.id, content, mainPanelEl);
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
        invoke('crit_overlay_dismissed', { ptyId }).catch((e: unknown) => {
          logToFile('WARN', `crit_overlay_dismissed failed: ${e}`);
        });
      }

      tabBar.render();
    }
  };

  tabBar.render();

  wsHub.load();
  overlayManager.showTabCoveringOverlay(wsHub.getElement());

  (window as any).__test = { sessionManager, overlayManager, tabBar, wsHub };
});
