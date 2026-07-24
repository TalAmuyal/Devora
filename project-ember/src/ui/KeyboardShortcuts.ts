import { SessionManager } from '../session/SessionManager';

const SHIFT_SHIFT_THRESHOLD_MS = 500;

type FontSizeAction = { kind: 'set'; px: number } | { kind: 'delta'; px: number };

/**
 * The font-size base shortcuts (Ctrl+1/2/3, Ctrl+=, Ctrl+Shift+±) mapped to their size change, or null.
 * The single source of truth for "this is a font shortcut": `handleGlobal` acts on it and `allowedThroughPageBarrier` admits it, so the two can never drift.
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

/**
 * The base (empty-stack / through-a-page-barrier) global shortcuts, driven by the LayerStack (ADR-003).
 *
 * This owns no `window`-capture keydown listener of its own: `observeKey` is registered as the stack's key observer (Shift-Shift tracking; never consumes) and `handleGlobal` as the stack's global handler (returns `true` when it consumes).
 * Only the keyup listener for the Shift-Shift double-tap stays here.
 */
export class KeyboardShortcuts {
  private sessionManager: SessionManager;
  private onOpenWsHub: () => void;
  private onOpenCommandPalette: () => void;
  private onOpenUserGuide: () => void;

  // Shift-Shift detection: the time the previous lone-Shift tap was released, and whether the currently-held Shift has stayed "lone" (no other key pressed while it was down)
  private lastShiftTapTime = 0;
  private shiftIsLone = false;

  constructor(
    sessionManager: SessionManager,
    onOpenWsHub: () => void,
    onOpenCommandPalette: () => void,
    onOpenUserGuide: () => void,
  ) {
    this.sessionManager = sessionManager;
    this.onOpenWsHub = onOpenWsHub;
    this.onOpenCommandPalette = onOpenCommandPalette;
    this.onOpenUserGuide = onOpenUserGuide;

    window.addEventListener('keyup', (e) => this.handleKeyUp(e), true);
  }

  /**
   * The LayerStack's key observer: sees every keydown and never consumes.
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

  /** The LayerStack's global handler for base shortcuts. Returns `true` when it consumes the key. */
  handleGlobal(e: KeyboardEvent): boolean {
    // Use e.code (physical key) for matching, because macOS WKWebView transforms e.key into control characters when Ctrl is held
    const code = e.code;
    const ctrl = e.ctrlKey;
    const shift = e.shiftKey;

    if (e.key === 'F1') {
      this.onOpenUserGuide();
      return true;
    }
    if (ctrl && !shift && code === 'KeyS') {
      this.onOpenWsHub();
      return true;
    }
    if (ctrl && shift && code === 'KeyS') {
      this.sessionManager.createSession();
      return true;
    }
    if (ctrl && !shift && code === 'ArrowLeft') {
      this.sessionManager.activatePrevious();
      return true;
    }
    if (ctrl && !shift && code === 'ArrowRight') {
      this.sessionManager.activateNext();
      return true;
    }
    if (ctrl && shift && code === 'ArrowLeft') {
      this.sessionManager.moveTabBackward();
      return true;
    }
    if (ctrl && shift && code === 'ArrowRight') {
      this.sessionManager.moveTabForward();
      return true;
    }
    const font = matchFontSizeShortcut(e);
    if (font) {
      if (font.kind === 'set') this.setFontSize(font.px);
      else this.changeFontSize(font.px);
      return true;
    }
    return false;
  }

  /**
   * Which base shortcuts stay live through a `page` barrier (ADR-003): F1 (User Guide) and font sizing.
   * New session (Ctrl+Shift+S) and tab switch/move are intentionally blocked under a page — they would act on the hidden tab bar/terminals.
   */
  allowedThroughPageBarrier(e: KeyboardEvent): boolean {
    return e.key === 'F1' || matchFontSizeShortcut(e) !== null;
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
        this.onOpenCommandPalette();
        return;
      }
      this.lastShiftTapTime = now;
    }
    // This Shift press is consumed; the next press starts a fresh candidate.
    this.shiftIsLone = false;
  }

  private changeFontSize(delta: number): void {
    const session = this.sessionManager.getActiveSession();
    if (!session) return;
    const current = session.terminalPane.getFontSize();
    const newSize = Math.max(8, Math.min(40, current + delta));
    this.setFontSize(newSize);
  }

  private setFontSize(size: number): void {
    document.documentElement.style.fontSize = `${size}px`;
    for (const session of this.sessionManager.getSessions()) {
      session.terminalPane.setFontSize(size);
    }
  }
}
