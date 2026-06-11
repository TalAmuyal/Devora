import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { KeyboardShortcuts } from '../KeyboardShortcuts';
import type { SessionManager } from '../../session/SessionManager';
import type { OverlayManager } from '../OverlayManager';

interface Harness {
  shortcuts: KeyboardShortcuts;
  createSession: ReturnType<typeof vi.fn>;
  onToggleWsHub: ReturnType<typeof vi.fn>;
  onOpenCommandPalette: ReturnType<typeof vi.fn>;
  onOpenUserGuide: ReturnType<typeof vi.fn>;
  dismissActiveOverlay: ReturnType<typeof vi.fn>;
}

function makeHarness(): Harness {
  const createSession = vi.fn();
  const onToggleWsHub = vi.fn();
  const onOpenCommandPalette = vi.fn();
  const onOpenUserGuide = vi.fn();
  const dismissActiveOverlay = vi.fn(() => false);

  const sessionManager = {
    createSession,
    getActiveSession: () => null,
    getSessions: () => [],
  } as unknown as SessionManager;

  const overlayManager = {
    dismissActiveOverlay,
  } as unknown as OverlayManager;

  const shortcuts = new KeyboardShortcuts(
    sessionManager,
    overlayManager,
    onToggleWsHub,
    onOpenCommandPalette,
    onOpenUserGuide,
  );

  return {
    shortcuts,
    createSession,
    onToggleWsHub,
    onOpenCommandPalette,
    onOpenUserGuide,
    dismissActiveOverlay,
  };
}

function keydown(init: KeyboardEventInit): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init }));
}

function keyup(init: KeyboardEventInit): void {
  window.dispatchEvent(new KeyboardEvent('keyup', { bubbles: true, cancelable: true, ...init }));
}

describe('KeyboardShortcuts', () => {
  let harness: Harness;

  beforeEach(() => {
    harness = makeHarness();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('Ctrl+S toggles the Workspace Hub', () => {
    keydown({ key: 's', code: 'KeyS', ctrlKey: true });
    expect(harness.onToggleWsHub).toHaveBeenCalledOnce();
    expect(harness.onOpenCommandPalette).not.toHaveBeenCalled();
  });

  it('Ctrl+Shift+S creates a new session', () => {
    keydown({ key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true });
    expect(harness.createSession).toHaveBeenCalledOnce();
  });

  it('shift+shift (rapid double-tap) opens the Command Palette, not the Hub', () => {
    keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
    keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });

    expect(harness.onOpenCommandPalette).toHaveBeenCalledOnce();
    expect(harness.onToggleWsHub).not.toHaveBeenCalled();
  });

  it('two Shift taps further apart than the threshold do not open the palette', () => {
    vi.useFakeTimers();
    keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
    vi.advanceTimersByTime(600);
    keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });

    expect(harness.onOpenCommandPalette).not.toHaveBeenCalled();
  });

  it('fires again on a fresh double-tap after an intervening key (e.g. dismissing the palette)', () => {
    const doubleTap = (): void => {
      keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
      keyup({ key: 'Shift', code: 'ShiftLeft' });
      keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
      keyup({ key: 'Shift', code: 'ShiftLeft' });
    };

    doubleTap();
    expect(harness.onOpenCommandPalette).toHaveBeenCalledTimes(1);

    // Dismiss the palette with Escape — a non-Shift key in between attempts.
    keydown({ key: 'Escape', code: 'Escape' });
    keyup({ key: 'Escape', code: 'Escape' });

    // The very next double-tap must work, with no wasted/extra Shift tap.
    doubleTap();
    expect(harness.onOpenCommandPalette).toHaveBeenCalledTimes(2);
  });

  it('a non-Shift key between Shift taps cancels the double-tap', () => {
    keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });
    keydown({ key: 'a', code: 'KeyA' });
    keyup({ key: 'a', code: 'KeyA' });
    keydown({ key: 'Shift', code: 'ShiftLeft', shiftKey: true });
    keyup({ key: 'Shift', code: 'ShiftLeft' });

    expect(harness.onOpenCommandPalette).not.toHaveBeenCalled();
  });

  it('F1 opens the User Guide', () => {
    keydown({ key: 'F1', code: 'F1' });
    expect(harness.onOpenUserGuide).toHaveBeenCalledOnce();
  });

  it('q attempts overlay dismissal when no editable element is focused', () => {
    keydown({ key: 'q', code: 'KeyQ' });
    expect(harness.dismissActiveOverlay).toHaveBeenCalledOnce();
  });

  it('q does not dismiss an overlay while a contentEditable element is focused', () => {
    const editable = document.createElement('div');
    editable.contentEditable = 'true';
    document.body.appendChild(editable);
    editable.focus();
    expect(document.activeElement).toBe(editable);

    keydown({ key: 'q', code: 'KeyQ' });
    expect(harness.dismissActiveOverlay).not.toHaveBeenCalled();

    editable.blur();
    editable.remove();
  });
});
