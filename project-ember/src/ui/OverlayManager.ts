/** Anything that can take keyboard focus, e.g. a terminal pane or search input */
export interface Focusable {
  focus(): void;
}

export class OverlayManager {
  private appEl: HTMLElement;

  // Tab-covering overlay state
  private tabCoveringOverlayEl: HTMLElement | null = null;
  private tabCoveringContentEl: HTMLElement | null = null;
  private tabCoveringCleanup: (() => void) | null = null;
  private tabCoveringRestoreFocusTo: Focusable | null = null;
  private tabCoveringUserDismiss: (() => void) | null = null;

  constructor(appEl: HTMLElement) {
    this.appEl = appEl;
  }

  // --- Tab-covering overlay ---

  showTabCoveringOverlay(
    content: HTMLElement,
    onCleanup?: () => void,
    restoreFocusTo?: Focusable | null,
    overlayClass?: string,
    onUserDismiss?: (() => void) | null,
  ): void {
    this.dismissTabCoveringOverlay();

    this.tabCoveringCleanup = onCleanup ?? null;
    this.tabCoveringRestoreFocusTo = restoreFocusTo ?? null;
    this.tabCoveringUserDismiss = onUserDismiss ?? null;
    this.tabCoveringOverlayEl = document.createElement('div');
    this.tabCoveringOverlayEl.className = 'overlay-tab-covering';
    // Focusable (out of the tab order) so we can move keyboard focus onto the overlay below — see the focus() call
    this.tabCoveringOverlayEl.tabIndex = -1;
    if (overlayClass) {
      this.tabCoveringOverlayEl.classList.add(overlayClass);
    }

    this.tabCoveringContentEl = content;
    this.tabCoveringOverlayEl.appendChild(content);
    this.appEl.appendChild(this.tabCoveringOverlayEl);

    /*
     * Take keyboard focus away from whatever held it (typically the terminal's textarea).
     * Without this the overlay's own key handling — and the global q/Escape-to-dismiss — would be suppressed by the editable-element guard until the user clicked the overlay.
     * Focusing the wrapper (rather than the content) keeps this concern in one place for every tab-covering overlay.
     */
    this.tabCoveringOverlayEl.focus();
  }

  dismissTabCoveringOverlay(): void {
    if (this.tabCoveringOverlayEl) {
      const cleanup = this.tabCoveringCleanup;
      const restoreFocusTo = this.tabCoveringRestoreFocusTo;
      this.tabCoveringCleanup = null;
      this.tabCoveringRestoreFocusTo = null;
      this.tabCoveringUserDismiss = null;
      this.tabCoveringOverlayEl.remove();
      this.tabCoveringOverlayEl = null;
      this.tabCoveringContentEl = null;
      try {
        cleanup?.();
      } catch (e) {
        console.error('Tab-covering overlay cleanup failed:', e);
      }
      restoreFocusTo?.focus();
    }
  }

  isTabCoveringOverlayActive(): boolean {
    return this.tabCoveringOverlayEl !== null;
  }

  // --- General ---

  dismissActiveOverlay(): boolean {
    if (this.tabCoveringOverlayEl) {
      // A user-dismiss override owns the decision: it may veto the dismissal (e.g. zero-profile lock) or replace this overlay with another one.
      // Programmatic teardown must use dismissTabCoveringOverlay() directly.
      if (this.tabCoveringUserDismiss) {
        this.tabCoveringUserDismiss();
        return true;
      }
      this.dismissTabCoveringOverlay();
      return true;
    }
    return false;
  }

  hasActiveOverlay(): boolean {
    return this.tabCoveringOverlayEl !== null;
  }
}
