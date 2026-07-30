import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { KeyboardShortcuts } from '../KeyboardShortcuts';
import type { SessionManager } from '../../session/SessionManager';

interface Harness {
  shortcuts: KeyboardShortcuts;
  createSession: ReturnType<typeof vi.fn>;
  activateNext: ReturnType<typeof vi.fn>;
  activatePrevious: ReturnType<typeof vi.fn>;
  onOpenWsHub: ReturnType<typeof vi.fn>;
  onOpenCommandPalette: ReturnType<typeof vi.fn>;
  onOpenUserGuide: ReturnType<typeof vi.fn>;
  /** Stand-in for "an opaque layer occupies the window stack"; flip it to test a shortcut's precondition. */
  windowSurface: { open: boolean };
}

function makeHarness(): Harness {
  const createSession = vi.fn();
  const activateNext = vi.fn();
  const activatePrevious = vi.fn();
  const onOpenWsHub = vi.fn();
  const onOpenCommandPalette = vi.fn();
  const onOpenUserGuide = vi.fn();
  const windowSurface = { open: false };

  const sessionManager = {
    createSession,
    activateNext,
    activatePrevious,
    getActiveSession: () => null,
    getSessions: () => [],
  } as unknown as SessionManager;

  const shortcuts = new KeyboardShortcuts({
    sessionManager,
    onOpenWsHub,
    onOpenCommandPalette,
    onOpenUserGuide,
    hasWindowSurface: () => windowSurface.open,
  });

  return {
    shortcuts,
    createSession,
    activateNext,
    activatePrevious,
    onOpenWsHub,
    onOpenCommandPalette,
    onOpenUserGuide,
    windowSurface,
  };
}

/** Drive the shortcut handler the way the router does, before any layer sees the key. Returns whether it consumed. */
function global(shortcuts: KeyboardShortcuts, init: KeyboardEventInit): boolean {
  return shortcuts.handleShortcut(new KeyboardEvent('keydown', init));
}

/** Feed a keydown to the key observer (the router calls this on every keydown; it never consumes). */
function observe(shortcuts: KeyboardShortcuts, init: KeyboardEventInit): void {
  shortcuts.observeKey(new KeyboardEvent('keydown', init));
}

/** Shift-Shift fires on keyup, which KeyboardShortcuts still owns as a window listener. */
function keyup(init: KeyboardEventInit): void {
  window.dispatchEvent(new KeyboardEvent('keyup', { bubbles: true, cancelable: true, ...init }));
}

describe('KeyboardShortcuts — shortcut handler', () => {
  let h: Harness;

  beforeEach(() => {
    h = makeHarness();
  });

  it('Ctrl+S opens the Workspace Hub and consumes the key', () => {
    expect(global(h.shortcuts, { key: 's', code: 'KeyS', ctrlKey: true })).toBe(true);
    expect(h.onOpenWsHub).toHaveBeenCalledOnce();
    expect(h.onOpenCommandPalette).not.toHaveBeenCalled();
  });

  it('Ctrl+Shift+S creates a new session', () => {
    expect(global(h.shortcuts, { key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true })).toBe(true);
    expect(h.createSession).toHaveBeenCalledOnce();
  });

  it('Ctrl+ArrowRight/Left switch sessions', () => {
    expect(global(h.shortcuts, { key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true })).toBe(true);
    expect(h.activateNext).toHaveBeenCalledOnce();
    expect(global(h.shortcuts, { key: 'ArrowLeft', code: 'ArrowLeft', ctrlKey: true })).toBe(true);
    expect(h.activatePrevious).toHaveBeenCalledOnce();
  });

  it('F1 opens the User Guide', () => {
    expect(global(h.shortcuts, { key: 'F1', code: 'F1' })).toBe(true);
    expect(h.onOpenUserGuide).toHaveBeenCalledOnce();
  });

  it('leaves an unhandled key for the focused terminal (returns false, no action)', () => {
    expect(global(h.shortcuts, { key: 'a', code: 'KeyA' })).toBe(false);
    expect(h.onOpenWsHub).not.toHaveBeenCalled();
    expect(h.createSession).not.toHaveBeenCalled();
  });
});

describe('KeyboardShortcuts — font sizing', () => {
  let h: Harness;

  beforeEach(() => {
    h = makeHarness();
    document.documentElement.style.fontSize = '';
  });

  it('Ctrl+1/2/3 set absolute UI sizes', () => {
    expect(global(h.shortcuts, { key: '1', code: 'Digit1', ctrlKey: true })).toBe(true);
    expect(document.documentElement.style.fontSize).toBe('12px');
    global(h.shortcuts, { key: '2', code: 'Digit2', ctrlKey: true });
    expect(document.documentElement.style.fontSize).toBe('15px');
    global(h.shortcuts, { key: '3', code: 'Digit3', ctrlKey: true });
    expect(document.documentElement.style.fontSize).toBe('26px');
  });

  it('Ctrl+= resets UI size to the default', () => {
    document.documentElement.style.fontSize = '99px';
    expect(global(h.shortcuts, { key: '=', code: 'Equal', ctrlKey: true })).toBe(true);
    expect(document.documentElement.style.fontSize).toBe('15px');
  });

  it('Ctrl+Shift+± consume even with no active session to resize', () => {
    expect(
      global(h.shortcuts, { key: '+', code: 'Equal', ctrlKey: true, shiftKey: true }),
    ).toBe(true);
    expect(
      global(h.shortcuts, { key: '_', code: 'Minus', ctrlKey: true, shiftKey: true }),
    ).toBe(true);
  });
});

describe('KeyboardShortcuts — preconditions under a window surface', () => {
  let h: Harness;

  beforeEach(() => {
    h = makeHarness();
    h.windowSurface.open = true;
    document.documentElement.style.fontSize = '';
  });

  it('font sizing and F1 stay live — a display preference and help are never unavailable', () => {
    expect(global(h.shortcuts, { key: '1', code: 'Digit1', ctrlKey: true })).toBe(true);
    expect(document.documentElement.style.fontSize).toBe('12px');
    expect(global(h.shortcuts, { key: 'F1', code: 'F1' })).toBe(true);
    expect(h.onOpenUserGuide).toHaveBeenCalledOnce();
  });

  it('consumes the surface shortcuts as no-ops rather than letting them fall through', () => {
    // Consuming matters: Ctrl+S must not reach the hub's own key handling, and none of these may reach a terminal.
    expect(global(h.shortcuts, { key: 's', code: 'KeyS', ctrlKey: true })).toBe(true);
    expect(global(h.shortcuts, { key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true })).toBe(true);
    expect(global(h.shortcuts, { key: 'ArrowLeft', code: 'ArrowLeft', ctrlKey: true })).toBe(true);
    expect(global(h.shortcuts, { key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true })).toBe(true);
    expect(global(h.shortcuts, { key: 'ArrowLeft', code: 'ArrowLeft', ctrlKey: true, shiftKey: true })).toBe(true);
    expect(global(h.shortcuts, { key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true, shiftKey: true })).toBe(true);

    expect(h.onOpenWsHub).not.toHaveBeenCalled();
    expect(h.createSession).not.toHaveBeenCalled();
    expect(h.activateNext).not.toHaveBeenCalled();
    expect(h.activatePrevious).not.toHaveBeenCalled();
  });

  it('leaves keys it does not claim alone, so the surface can handle them', () => {
    expect(global(h.shortcuts, { key: 'n', code: 'KeyN' })).toBe(false);
    expect(global(h.shortcuts, { key: 'j', code: 'KeyJ' })).toBe(false);
  });
});

describe('KeyboardShortcuts — Shift-Shift double-tap', () => {
  let h: Harness;

  beforeEach(() => {
    h = makeHarness();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  const doubleTap = (shortcuts: KeyboardShortcuts): void => {
    observe(shortcuts, { key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
    observe(shortcuts, { key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
  };

  it('a rapid double-tap opens the Command Palette, not the Hub', () => {
    doubleTap(h.shortcuts);
    expect(h.onOpenCommandPalette).toHaveBeenCalledOnce();
    expect(h.onOpenWsHub).not.toHaveBeenCalled();
  });

  it('two Shift taps further apart than the threshold do not open the palette', () => {
    vi.useFakeTimers();
    observe(h.shortcuts, { key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
    vi.advanceTimersByTime(600);
    observe(h.shortcuts, { key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });

    expect(h.onOpenCommandPalette).not.toHaveBeenCalled();
  });

  it('fires again on a fresh double-tap after an intervening key (e.g. dismissing the palette)', () => {
    doubleTap(h.shortcuts);
    expect(h.onOpenCommandPalette).toHaveBeenCalledTimes(1);

    // An intervening non-Shift key (the Escape/q that dismisses the palette).
    observe(h.shortcuts, { key: 'Escape', code: 'Escape' });
    keyup({ key: 'Escape', code: 'Escape' });

    doubleTap(h.shortcuts);
    expect(h.onOpenCommandPalette).toHaveBeenCalledTimes(2);
  });

  it('does not open the palette while a window surface is up', () => {
    h.windowSurface.open = true;
    doubleTap(h.shortcuts);
    expect(h.onOpenCommandPalette).not.toHaveBeenCalled();
  });

  it('a non-Shift key between Shift taps cancels the double-tap', () => {
    observe(h.shortcuts, { key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
    observe(h.shortcuts, { key: 'a', code: 'KeyA' });
    keyup({ key: 'a', code: 'KeyA' });
    observe(h.shortcuts, { key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });

    expect(h.onOpenCommandPalette).not.toHaveBeenCalled();
  });
});
