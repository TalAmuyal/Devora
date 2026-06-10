import { SessionManager } from '../session/SessionManager';
import { OverlayManager } from './OverlayManager';
import { isEditableElementFocused } from './focus';

const SHIFT_SHIFT_THRESHOLD_MS = 500;

export class KeyboardShortcuts {
  private sessionManager: SessionManager;
  private overlayManager: OverlayManager;
  private onToggleWsHub: () => void;
  private onOpenCommandPalette: () => void;
  private onOpenUserGuide: () => void;

  // Shift-Shift detection: the time the previous lone-Shift tap was released, and whether the currently-held Shift has stayed "lone" (no other key pressed while it was down)
  private lastShiftTapTime = 0;
  private shiftIsLone = false;

  constructor(
    sessionManager: SessionManager,
    overlayManager: OverlayManager,
    onToggleWsHub: () => void,
    onOpenCommandPalette: () => void,
    onOpenUserGuide: () => void,
  ) {
    this.sessionManager = sessionManager;
    this.overlayManager = overlayManager;
    this.onToggleWsHub = onToggleWsHub;
    this.onOpenCommandPalette = onOpenCommandPalette;
    this.onOpenUserGuide = onOpenUserGuide;

    window.addEventListener('keydown', (e) => this.handleKeyDown(e), true);
    window.addEventListener('keyup', (e) => this.handleKeyUp(e), true);
  }

  private handleKeyDown(e: KeyboardEvent): void {
    /*
     * Track Shift-Shift (a rapid double-tap of a lone Shift).
     * A tap only counts if Shift was pressed and released with no other key in between.
     * We decide "lone" on the Shift *press* (ignoring auto-repeat) rather than restoring a flag after a release — that's what makes the next double-tap fire reliably right after an intervening key, such as the Escape/q that dismisses the palette.
     * Any non-Shift key both breaks the current press and discards a pending first tap.
     */
    if (e.key === 'Shift') {
      if (!e.repeat) {
        this.shiftIsLone = true;
      }
    } else {
      this.shiftIsLone = false;
      this.lastShiftTapTime = 0;
    }

    // Use e.code (physical key) for matching, because macOS WKWebView transforms e.key into control characters when Ctrl is held
    const code = e.code;
    const ctrl = e.ctrlKey;
    const shift = e.shiftKey;

    if (e.key === 'Escape' || e.key === 'q') {
      if (isEditableElementFocused()) {
        return;
      }
      if (this.overlayManager.dismissActiveOverlay()) {
        e.preventDefault();
        e.stopPropagation();
        return;
      }
    }

    if (e.key === 'F1') {
      e.preventDefault();
      e.stopPropagation();
      this.onOpenUserGuide();
      return;
    }

    if (ctrl && !shift && code === 'KeyS') {
      e.preventDefault();
      e.stopPropagation();
      this.onToggleWsHub();
      return;
    }

    if (ctrl && shift && code === 'KeyS') {
      e.preventDefault();
      e.stopPropagation();
      this.sessionManager.createSession();
      return;
    }

    if (ctrl && !shift && code === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      this.sessionManager.activatePrevious();
      return;
    }

    if (ctrl && !shift && code === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      this.sessionManager.activateNext();
      return;
    }

    if (ctrl && shift && code === 'ArrowLeft') {
      e.preventDefault();
      e.stopPropagation();
      this.sessionManager.moveTabBackward();
      return;
    }

    if (ctrl && shift && code === 'ArrowRight') {
      e.preventDefault();
      e.stopPropagation();
      this.sessionManager.moveTabForward();
      return;
    }

    if (ctrl && shift && (code === 'Equal' || code === 'NumpadAdd')) {
      e.preventDefault();
      e.stopPropagation();
      this.changeFontSize(2);
      return;
    }

    if (ctrl && !shift && code === 'Equal') {
      e.preventDefault();
      e.stopPropagation();
      this.setFontSize(15);
      return;
    }

    if (ctrl && shift && (code === 'Minus' || code === 'NumpadSubtract')) {
      e.preventDefault();
      e.stopPropagation();
      this.changeFontSize(-2);
      return;
    }

    if (ctrl && !shift && code === 'Digit1') {
      e.preventDefault();
      e.stopPropagation();
      this.setFontSize(12);
      return;
    }
    if (ctrl && !shift && code === 'Digit2') {
      e.preventDefault();
      e.stopPropagation();
      this.setFontSize(15);
      return;
    }
    if (ctrl && !shift && code === 'Digit3') {
      e.preventDefault();
      e.stopPropagation();
      this.setFontSize(26);
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
    this.applySize(newSize);
  }

  private setFontSize(size: number): void {
    this.applySize(size);
  }

  private applySize(size: number): void {
    document.documentElement.style.fontSize = `${size}px`;
    for (const session of this.sessionManager.getSessions()) {
      session.terminalPane.setFontSize(size);
    }
  }
}
