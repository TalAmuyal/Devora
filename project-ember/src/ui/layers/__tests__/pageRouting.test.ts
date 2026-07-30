import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LayerRouter } from '../LayerRouter';
import { LayerStack } from '../LayerStack';
import { modalLayer, pageLayer } from '../presets';
import type { DismissDecision, LayerHandle, LayerSpec } from '../types';
import { KeyboardShortcuts } from '../../KeyboardShortcuts';
import { WorkspaceHub } from '../../../workspace/WorkspaceHub';
import type { SessionManager } from '../../../session/SessionManager';

/**
 * The surfaces `main.ts` wires, routed through a real router and a real `KeyboardShortcuts` (ADR-003).
 * `main.ts` is not unit-testable itself, so these pin its wiring decisions: dismissal semantics, and which
 * app-wide shortcuts stay live under a surface.
 *
 * The Workspace Hub is the *real* one, not a stand-in: its key handler claims bare `1`/`2`/`3` as category
 * filters, which is exactly what a stand-in would fail to reproduce.
 */

vi.mock('../../../invoke', () => ({
  invoke: vi.fn(async () => []),
  invokeLogOnly: vi.fn(async () => []),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

let router: LayerRouter | null = null;
let windowStack: LayerStack;
let tabStack: LayerStack;
let onEmptied: ReturnType<typeof vi.fn>;

function install(): void {
  const windowHost = document.createElement('div');
  const tabHost = document.createElement('div');
  document.body.append(windowHost, tabHost);
  onEmptied = vi.fn();
  windowStack = new LayerStack({ host: windowHost, onEmptied });
  tabStack = new LayerStack({ host: tabHost });
  router = new LayerRouter({ windowStack, resolveTabStack: () => tabStack });
  router.install();
}

function content(name: string): HTMLElement {
  const el = document.createElement('div');
  el.className = `${name}-content`;
  return el;
}

function dispatch(init: KeyboardEventInit): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(e);
  return e;
}

interface FakeSession {
  setFontSize: ReturnType<typeof vi.fn>;
  getFontSize: () => number;
}

interface ShortcutsHarness {
  shortcuts: KeyboardShortcuts;
  createSession: ReturnType<typeof vi.fn>;
  activateNext: ReturnType<typeof vi.fn>;
  openWsHub: ReturnType<typeof vi.fn>;
  openUserGuide: ReturnType<typeof vi.fn>;
  session: FakeSession;
}

/** Wire KeyboardShortcuts to the window stack exactly as `main.ts` does. */
function wireShortcuts(): ShortcutsHarness {
  const session: FakeSession = { setFontSize: vi.fn(), getFontSize: () => 15 };
  const createSession = vi.fn();
  const activateNext = vi.fn();
  const sessionManager = {
    createSession,
    activateNext,
    activatePrevious: vi.fn(),
    moveTabForward: vi.fn(),
    moveTabBackward: vi.fn(),
    getActiveSession: () => session,
    getSessions: () => [session],
  } as unknown as SessionManager;

  const openWsHub = vi.fn();
  const openUserGuide = vi.fn();
  const shortcuts = new KeyboardShortcuts({
    sessionManager,
    onOpenWsHub: openWsHub,
    onOpenCommandPalette: vi.fn(),
    onOpenUserGuide: openUserGuide,
    hasWindowSurface: () => windowStack.hasOpaqueLayer(),
  });
  router!.setShortcutHandler((e) => shortcuts.handleShortcut(e));
  return { shortcuts, createSession, activateNext, openWsHub, openUserGuide, session };
}

/** The real Workspace Hub, pushed the way `main.ts` pushes it. */
function pushRealHub(opts: { onReveal?: () => void; dismiss?: () => DismissDecision } = {}): {
  hub: WorkspaceHub;
  handle: LayerHandle;
} {
  const hub = new WorkspaceHub(vi.fn(), vi.fn(), vi.fn(), vi.fn(), vi.fn(), vi.fn());
  const handle = windowStack.push(
    pageLayer({
      name: 'ws-hub',
      element: hub.getElement(),
      onKey: (e) => hub.handleKey(e),
      onUserDismissRequest: opts.dismiss ?? (() => hub.handleUserDismiss()),
      onReveal: opts.onReveal,
    }),
  );
  return { hub, handle };
}

function openSettings(): LayerHandle {
  return windowStack.push(pageLayer({ name: 'settings-hub', element: content('settings-hub') }));
}

beforeEach(() => {
  install();
  document.documentElement.style.fontSize = '';
});

afterEach(() => {
  router?.uninstall();
  router = null;
  windowStack.clear();
  tabStack.clear();
  document.body.innerHTML = '';
});

describe('page routing — Settings over the hub', () => {
  it('opening Settings over the hub then dismissing pops back and refreshes the revealed hub', () => {
    const onReveal = vi.fn();
    const { handle: hub } = pushRealHub({ onReveal });

    const settings = openSettings();
    expect(windowStack.top()).toBe(settings);
    expect(windowStack.depth()).toBe(2);

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(windowStack.find('settings-hub')).toBeNull();
    expect(windowStack.top()).toBe(hub);
    expect(onReveal).toHaveBeenCalledOnce();
  });

  it('dismissing Settings opened with no hub beneath empties the stack', () => {
    openSettings(); // palette path: nothing beneath

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(windowStack.isEmpty()).toBe(true);
    expect(onEmptied).toHaveBeenCalledOnce();
  });
});

describe('page routing — hub dismissal decisions', () => {
  it('the zero-profile hub vetoes q and stays open', () => {
    pushRealHub({ dismiss: () => 'veto' });

    const e = dispatch({ key: 'q', code: 'KeyQ' });

    expect(windowStack.find('ws-hub')).not.toBeNull();
    expect(windowStack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
  });

  it('closing the hub cheatsheet with q fires once, consumes the key, and keeps the hub', () => {
    let cheatsheetOpen = true;
    const dismiss = vi.fn((): DismissDecision => {
      if (cheatsheetOpen) {
        cheatsheetOpen = false;
        return 'handled';
      }
      return 'close';
    });
    pushRealHub({ dismiss });

    const e = dispatch({ key: 'q', code: 'KeyQ' });

    expect(dismiss).toHaveBeenCalledOnce();
    expect(cheatsheetOpen).toBe(false);
    expect(windowStack.depth()).toBe(1); // the hub is not closed by the same press
    expect(e.defaultPrevented).toBe(true);
  });
});

describe('page routing — app-wide shortcuts under a surface', () => {
  it('font sizing works inside the Workspace Hub, whose own handler claims bare 1/2/3', () => {
    const { session } = wireShortcuts();
    pushRealHub();

    dispatch({ key: '1', code: 'Digit1', ctrlKey: true });

    expect(document.documentElement.style.fontSize).toBe('12px');
    expect(session.setFontSize).toHaveBeenCalledWith(12);
  });

  it('font sizing works under a modal, which used to block everything', () => {
    const { session } = wireShortcuts();
    windowStack.push(modalLayer({ name: 'confirm', element: content('confirm') }));

    dispatch({ key: '3', code: 'Digit3', ctrlKey: true });

    expect(document.documentElement.style.fontSize).toBe('26px');
    expect(session.setFontSize).toHaveBeenCalledWith(26);
  });

  it('F1 reaches the User Guide from under a modal', () => {
    const { openUserGuide } = wireShortcuts();
    windowStack.push(modalLayer({ name: 'confirm', element: content('confirm') }));

    const e = dispatch({ key: 'F1', code: 'F1' });

    expect(openUserGuide).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('new session and tab navigation are inert under a page but live on an empty stack', () => {
    const { createSession, activateNext } = wireShortcuts();
    const { handle: hub } = pushRealHub();

    dispatch({ key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true });
    dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });
    expect(createSession).not.toHaveBeenCalled();
    expect(activateNext).not.toHaveBeenCalled();

    windowStack.remove(hub);
    dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });
    expect(activateNext).toHaveBeenCalledOnce();
  });

  it('tab navigation stays live while only a tab-bound surface is open', () => {
    const { activateNext } = wireShortcuts();
    tabStack.push(pageLayer({ name: 'crit-review', element: content('crit') }));

    dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });

    // A Crit review covers its session, not the window — it has no business freezing the tab bar.
    expect(activateNext).toHaveBeenCalledOnce();
  });
});

describe('page routing — Ctrl+S opens only', () => {
  it('opens the hub on an empty stack', () => {
    const { openWsHub } = wireShortcuts();

    const e = dispatch({ key: 's', code: 'KeyS', ctrlKey: true });

    expect(openWsHub).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('is a consumed no-op while the hub is open — it never closes anything', () => {
    const { openWsHub } = wireShortcuts();
    pushRealHub();

    const e = dispatch({ key: 's', code: 'KeyS', ctrlKey: true });

    expect(windowStack.find('ws-hub')).not.toBeNull();
    expect(windowStack.depth()).toBe(1);
    expect(openWsHub).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(true);
  });

  it('is a consumed no-op under a modal', () => {
    const { openWsHub } = wireShortcuts();
    windowStack.push(modalLayer({ name: 'confirm', element: content('confirm') }));

    const e = dispatch({ key: 's', code: 'KeyS', ctrlKey: true });

    expect(openWsHub).not.toHaveBeenCalled();
    expect(windowStack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
  });
});

describe('page routing — the palette', () => {
  it('needs no Ctrl+S handler of its own: the precondition blocks the key structurally', () => {
    const { openWsHub } = wireShortcuts();
    const paletteSpec: LayerSpec = pageLayer({
      name: 'command-palette',
      element: content('command-palette'),
      wrapperClass: 'layer-transparent',
    });
    windowStack.push(paletteSpec);

    dispatch({ key: 's', code: 'KeyS', ctrlKey: true });

    expect(windowStack.find('command-palette')).not.toBeNull();
    expect(openWsHub).not.toHaveBeenCalled();
  });
});
