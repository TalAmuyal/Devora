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
}

function makeHarness(): Harness {
  const createSession = vi.fn();
  const activateNext = vi.fn();
  const activatePrevious = vi.fn();
  const onOpenWsHub = vi.fn();
  const onOpenCommandPalette = vi.fn();
  const onOpenUserGuide = vi.fn();

  const sessionManager = {
    createSession,
    activateNext,
    activatePrevious,
    getActiveSession: () => null,
    getSessions: () => [],
  } as unknown as SessionManager;

  const shortcuts = new KeyboardShortcuts(
    sessionManager,
    onOpenWsHub,
    onOpenCommandPalette,
    onOpenUserGuide,
  );

  return {
    shortcuts,
    createSession,
    activateNext,
    activatePrevious,
    onOpenWsHub,
    onOpenCommandPalette,
    onOpenUserGuide,
  };
}

/** Drive the global handler the way the LayerStack would when a key reaches the base. Returns whether it consumed the key. */
function global(shortcuts: KeyboardShortcuts, init: KeyboardEventInit): boolean {
  return shortcuts.handleGlobal(new KeyboardEvent('keydown', init));
}

/** Feed a keydown to the key observer (the LayerStack calls this on every keydown; it never consumes). */
function observe(shortcuts: KeyboardShortcuts, init: KeyboardEventInit): void {
  shortcuts.observeKey(new KeyboardEvent('keydown', init));
}

/** Shift-Shift fires on keyup, which KeyboardShortcuts still owns as a window listener. */
function keyup(init: KeyboardEventInit): void {
  window.dispatchEvent(new KeyboardEvent('keyup', { bubbles: true, cancelable: true, ...init }));
}

describe('KeyboardShortcuts — global handler', () => {
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
