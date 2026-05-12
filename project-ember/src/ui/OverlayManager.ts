export class OverlayManager {
  private appEl: HTMLElement;

  // Tab-covering overlay state
  private tabCoveringOverlayEl: HTMLElement | null = null;
  private tabCoveringContentEl: HTMLElement | null = null;

  // Panel overlays tied to session tabs (sessionId -> overlay element)
  private panelOverlays: Map<number, HTMLElement> = new Map();
  private activePanelOverlaySessionId: number | null = null;

  constructor(appEl: HTMLElement) {
    this.appEl = appEl;
  }

  // --- Tab-covering overlay ---

  showTabCoveringOverlay(content: HTMLElement): void {
    this.dismissTabCoveringOverlay();

    this.tabCoveringOverlayEl = document.createElement('div');
    this.tabCoveringOverlayEl.className = 'overlay-tab-covering';

    this.tabCoveringContentEl = content;
    this.tabCoveringOverlayEl.appendChild(content);
    this.appEl.appendChild(this.tabCoveringOverlayEl);
  }

  dismissTabCoveringOverlay(): void {
    if (this.tabCoveringOverlayEl) {
      this.tabCoveringOverlayEl.remove();
      this.tabCoveringOverlayEl = null;
      this.tabCoveringContentEl = null;
    }
  }

  isTabCoveringOverlayActive(): boolean {
    return this.tabCoveringOverlayEl !== null;
  }

  // --- Panel overlay (tied to session tab) ---

  showPanelOverlay(sessionId: number, content: HTMLElement, mainPanelEl: HTMLElement): void {
    this.dismissPanelOverlay(sessionId);

    const overlayEl = document.createElement('div');
    overlayEl.className = 'overlay-panel';
    overlayEl.appendChild(content);

    mainPanelEl.appendChild(overlayEl);
    this.panelOverlays.set(sessionId, overlayEl);
    this.activePanelOverlaySessionId = sessionId;
  }

  dismissPanelOverlay(sessionId: number): void {
    const overlayEl = this.panelOverlays.get(sessionId);
    if (overlayEl) {
      overlayEl.remove();
      this.panelOverlays.delete(sessionId);
      if (this.activePanelOverlaySessionId === sessionId) {
        this.activePanelOverlaySessionId = null;
      }
    }
  }

  // Called when session tabs switch — show/hide panel overlays accordingly
  onSessionActivated(sessionId: number): void {
    for (const [id, el] of this.panelOverlays) {
      el.style.display = id === sessionId ? 'block' : 'none';
    }
    this.activePanelOverlaySessionId = this.panelOverlays.has(sessionId) ? sessionId : null;
  }

  hasPanelOverlay(sessionId: number): boolean {
    return this.panelOverlays.has(sessionId);
  }

  // --- Popup / Dialog (deferred stubs) ---

  showPopup(_content: HTMLElement): void {
    console.warn('Popup overlay mode not yet implemented');
  }

  showDialog(_content: HTMLElement): void {
    console.warn('Dialog overlay mode not yet implemented');
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
