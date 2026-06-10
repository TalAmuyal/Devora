import {
  showConfirmationDialog,
  ConfirmationDialogOptions,
} from './components/ConfirmationDialog';

/** Anything that can take keyboard focus, e.g. a terminal pane or search input */
export interface Focusable {
  focus(): void;
}

interface PanelOverlayEntry {
  el: HTMLElement;
  restoreFocusTo: Focusable | null;
}

export class OverlayManager {
  private appEl: HTMLElement;

  // Tab-covering overlay state
  private tabCoveringOverlayEl: HTMLElement | null = null;
  private tabCoveringContentEl: HTMLElement | null = null;
  private tabCoveringCleanup: (() => void) | null = null;
  private tabCoveringRestoreFocusTo: Focusable | null = null;

  // Panel overlays tied to session tabs (sessionId -> overlay entry)
  private panelOverlays: Map<number, PanelOverlayEntry> = new Map();
  private activePanelOverlaySessionId: number | null = null;

  constructor(appEl: HTMLElement) {
    this.appEl = appEl;
  }

  // --- Tab-covering overlay ---

  showTabCoveringOverlay(
    content: HTMLElement,
    onCleanup?: () => void,
    restoreFocusTo?: Focusable | null,
    overlayClass?: string,
  ): void {
    this.dismissTabCoveringOverlay();

    this.tabCoveringCleanup = onCleanup ?? null;
    this.tabCoveringRestoreFocusTo = restoreFocusTo ?? null;
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

  // --- Panel overlay (tied to session tab) ---

  showPanelOverlay(
    sessionId: number,
    content: HTMLElement,
    mainPanelEl: HTMLElement,
    restoreFocusTo: Focusable | null,
  ): void {
    this.dismissPanelOverlay(sessionId);

    const overlayEl = document.createElement('div');
    overlayEl.className = 'overlay-panel';
    overlayEl.appendChild(content);

    mainPanelEl.appendChild(overlayEl);
    this.panelOverlays.set(sessionId, { el: overlayEl, restoreFocusTo });
    this.activePanelOverlaySessionId = sessionId;
  }

  dismissPanelOverlay(sessionId: number): void {
    const entry = this.panelOverlays.get(sessionId);
    if (entry) {
      // Only restore focus when dismissing the visible overlay: a backend-initiated close can target a hidden session, and refocusing it would steal focus.
      const wasActive = this.activePanelOverlaySessionId === sessionId;
      entry.el.remove();
      this.panelOverlays.delete(sessionId);
      if (this.activePanelOverlaySessionId === sessionId) {
        this.activePanelOverlaySessionId = null;
      }
      if (wasActive) {
        entry.restoreFocusTo?.focus();
      }
    }
  }

  // Called when session tabs switch — show/hide panel overlays accordingly
  onSessionActivated(sessionId: number): void {
    for (const [id, entry] of this.panelOverlays) {
      entry.el.style.display = id === sessionId ? 'block' : 'none';
    }
    this.activePanelOverlaySessionId = this.panelOverlays.has(sessionId) ? sessionId : null;
  }

  hasPanelOverlay(sessionId: number): boolean {
    return this.panelOverlays.has(sessionId);
  }

  isPanelOverlayVisible(sessionId: number): boolean {
    const entry = this.panelOverlays.get(sessionId);
    return entry !== undefined && entry.el.style.display !== 'none';
  }

  hasAnyVisiblePanelOverlay(): boolean {
    for (const entry of this.panelOverlays.values()) {
      if (entry.el.style.display !== 'none') return true;
    }
    return false;
  }

  // --- Popup / Dialog (deferred stubs) ---

  showPopup(_content: HTMLElement): void {
    console.warn('Popup overlay mode not yet implemented');
  }

  showDialog(options: ConfirmationDialogOptions): Promise<boolean> {
    return showConfirmationDialog(options);
  }

  // --- General ---

  dismissActiveOverlay(): boolean {
    if (this.tabCoveringOverlayEl) {
      this.dismissTabCoveringOverlay();
      return true;
    }
    if (this.activePanelOverlaySessionId !== null) {
      this.dismissPanelOverlay(this.activePanelOverlaySessionId);
      return true;
    }
    return false;
  }

  hasActiveOverlay(): boolean {
    return this.tabCoveringOverlayEl !== null || this.activePanelOverlaySessionId !== null;
  }
}
