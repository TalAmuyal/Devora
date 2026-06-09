import { SessionManager } from '../session/SessionManager';
import { OverlayManager } from './OverlayManager';

const SHIFT_SHIFT_THRESHOLD_MS = 500;

export class KeyboardShortcuts {
  private sessionManager: SessionManager;
  private overlayManager: OverlayManager;
  private onToggleWsHub: () => void;
  private onOpenUserGuide: () => void;

  private lastShiftUpTime = 0;
  private shiftWasAlone = true;

  constructor(
    sessionManager: SessionManager,
    overlayManager: OverlayManager,
    onToggleWsHub: () => void,
    onOpenUserGuide: () => void,
  ) {
    this.sessionManager = sessionManager;
    this.overlayManager = overlayManager;
    this.onToggleWsHub = onToggleWsHub;
    this.onOpenUserGuide = onOpenUserGuide;

    window.addEventListener('keydown', (e) => this.handleKeyDown(e), true);
    window.addEventListener('keyup', (e) => this.handleKeyUp(e), true);
  }

  private handleKeyDown(e: KeyboardEvent): void {
    if (e.key !== 'Shift') {
      this.shiftWasAlone = false;
    }

    // Use e.code (physical key) for matching, because macOS WKWebView
    // transforms e.key into control characters when Ctrl is held.
    const code = e.code;
    const ctrl = e.ctrlKey;
    const shift = e.shiftKey;

    if (e.key === 'Escape' || e.key === 'q') {
      const tag = document.activeElement?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
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
    if (e.key === 'Shift') {
      if (this.shiftWasAlone) {
        const now = Date.now();
        if (now - this.lastShiftUpTime < SHIFT_SHIFT_THRESHOLD_MS) {
          this.lastShiftUpTime = 0;
          this.onToggleWsHub();
          return;
        }
        this.lastShiftUpTime = now;
      }
      this.shiftWasAlone = true;
    } else {
      this.shiftWasAlone = false;
    }
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
