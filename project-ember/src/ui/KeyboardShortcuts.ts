import { SessionManager } from '../session/SessionManager';

const SHIFT_SHIFT_THRESHOLD_MS = 500;

type FontSizeAction = { kind: 'set'; px: number } | { kind: 'delta'; px: number };

/** The shortcuts that open a window surface or navigate tabs — the ones a window surface makes unavailable. */
type SurfaceShortcut =
  | 'workspace-hub'
  | 'new-session'
  | 'previous-tab'
  | 'next-tab'
  | 'move-tab-backward'
  | 'move-tab-forward';

/**
 * The font-size shortcuts (Ctrl+1/2/3, Ctrl+=, Ctrl+Shift+±) mapped to their size change, or null.
 * Matches on `e.code` (physical key) because macOS WKWebView transforms `e.key` into control characters when Ctrl is held.
 */
function matchFontSizeShortcut(e: KeyboardEvent): FontSizeAction | null {
  if (!e.ctrlKey) return null;
  const { shiftKey: shift, code } = e;
  if (shift && (code === 'Equal' || code === 'NumpadAdd')) return { kind: 'delta', px: 2 };
  if (shift && (code === 'Minus' || code === 'NumpadSubtract')) return { kind: 'delta', px: -2 };
  if (shift) return null;
  if (code === 'Equal' || code === 'Digit2') return { kind: 'set', px: 15 };
  if (code === 'Digit1') return { kind: 'set', px: 12 };
  if (code === 'Digit3') return { kind: 'set', px: 26 };
  return null;
}

function matchSurfaceShortcut(e: KeyboardEvent): SurfaceShortcut | null {
  if (!e.ctrlKey || e.metaKey || e.altKey) return null;
  const { shiftKey: shift, code } = e;
  if (code === 'KeyS') return shift ? 'new-session' : 'workspace-hub';
  if (code === 'ArrowLeft') return shift ? 'move-tab-backward' : 'previous-tab';
  if (code === 'ArrowRight') return shift ? 'move-tab-forward' : 'next-tab';
  return null;
}

export interface KeyboardShortcutsDeps {
  sessionManager: SessionManager;
  onOpenWsHub: () => void;
  onOpenCommandPalette: () => void;
  onOpenUserGuide: () => void;
  /**
   * Whether an opaque layer occupies the *window* stack — the single precondition shared by every shortcut that opens a window surface or navigates tabs.
   * Scoped to the window stack on purpose: a Crit review or a terminal lives in a tab stack and is opaque, and neither should make tab switching inert.
   */
  hasWindowSurface: () => boolean;
}

/**
 * The app-wide shortcut *policy*, and a participant in the layer system — see `./layers/CLAUDE.md` for the routing order and the availability table this file implements.
 *
 * `LayerRouter` runs it before any layer sees a key, so each shortcut states its own availability here instead of a surface deciding after the fact which app keys survive it.
 *
 * A shortcut whose precondition fails is still *consumed*, as a deliberate no-op: `Ctrl+S` with the hub open must not fall through to the hub's own key handling, and none of these keys should ever reach a terminal.
 *
 * This owns no keydown listener: `observeKey` is the router's key observer and `handleShortcut` its shortcut handler.
 * Only the keyup listener for the Shift-Shift double-tap stays here, since that gesture resolves on release.
 */
export class KeyboardShortcuts {
  private readonly deps: KeyboardShortcutsDeps;

  // Shift-Shift detection: the time the previous lone-Shift tap was released, and whether the currently-held Shift has stayed "lone" (no other key pressed while it was down)
  private lastShiftTapTime = 0;
  private shiftIsLone = false;

  constructor(deps: KeyboardShortcutsDeps) {
    this.deps = deps;
    window.addEventListener('keyup', (e) => this.handleKeyUp(e), true);
  }

  /**
   * The router's key observer: sees every keydown and never consumes.
   * Tracks Shift-Shift (a rapid double-tap of a lone Shift): a tap only counts if Shift was pressed and released with no other key in between.
   * We decide "lone" on the Shift *press* (ignoring auto-repeat) rather than restoring a flag after a release — that's what makes the next double-tap fire reliably right after an intervening key, such as the Escape/q that dismisses the palette.
   * Any non-Shift key both breaks the current press and discards a pending first tap.
   */
  observeKey(e: KeyboardEvent): void {
    if (e.key === 'Shift') {
      if (!e.repeat) {
        this.shiftIsLone = true;
      }
    } else {
      this.shiftIsLone = false;
      this.lastShiftTapTime = 0;
    }
  }

  /** Returns `true` when it consumes the key. */
  handleShortcut(e: KeyboardEvent): boolean {
    // Font size is a display preference and F1 is help: no surface has a reason to make either unavailable, so both run unconditionally.
    const font = matchFontSizeShortcut(e);
    if (font) {
      if (font.kind === 'set') this.setFontSize(font.px);
      else this.changeFontSize(font.px);
      return true;
    }
    if (e.key === 'F1') {
      // Re-entrancy (the guide already being open) is the callee's concern; the key is consumed either way.
      this.deps.onOpenUserGuide();
      return true;
    }

    const surface = matchSurfaceShortcut(e);
    if (!surface) return false;
    if (this.deps.hasWindowSurface()) return true;
    this.runSurfaceShortcut(surface);
    return true;
  }

  private runSurfaceShortcut(shortcut: SurfaceShortcut): void {
    const sessions = this.deps.sessionManager;
    switch (shortcut) {
      case 'workspace-hub':
        this.deps.onOpenWsHub();
        return;
      case 'new-session':
        void sessions.createSession();
        return;
      case 'previous-tab':
        sessions.activatePrevious();
        return;
      case 'next-tab':
        sessions.activateNext();
        return;
      case 'move-tab-backward':
        sessions.moveTabBackward();
        return;
      case 'move-tab-forward':
        sessions.moveTabForward();
        return;
    }
  }

  private handleKeyUp(e: KeyboardEvent): void {
    if (e.key !== 'Shift') {
      return;
    }
    if (this.shiftIsLone) {
      const now = Date.now();
      if (now - this.lastShiftTapTime < SHIFT_SHIFT_THRESHOLD_MS) {
        this.lastShiftTapTime = 0;
        this.shiftIsLone = false;
        // The palette is a window surface, so it obeys the same precondition — checked here because this gesture resolves on keyup, outside the router's keydown path.
        if (!this.deps.hasWindowSurface()) {
          this.deps.onOpenCommandPalette();
        }
        return;
      }
      this.lastShiftTapTime = now;
    }
    // This Shift press is consumed; the next press starts a fresh candidate.
    this.shiftIsLone = false;
  }

  private changeFontSize(delta: number): void {
    const session = this.deps.sessionManager.getActiveSession();
    if (!session) return;
    const current = session.getFontSize();
    const newSize = Math.max(8, Math.min(40, current + delta));
    this.setFontSize(newSize);
  }

  private setFontSize(size: number): void {
    document.documentElement.style.fontSize = `${size}px`;
    for (const session of this.deps.sessionManager.getSessions()) {
      session.setFontSize(size);
    }
  }
}
