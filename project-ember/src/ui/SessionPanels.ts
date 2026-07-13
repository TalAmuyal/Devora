/**
 * Owns the per-session panel overlays that cover the terminal region (Crit review, task-creation progress).
 *
 * Each session may have one panel.
 * A panel's content element is mounted in `mainPanelEl` for its whole lifetime (show → dismiss) and shown/hidden with `display`, so a live `<iframe>` (Crit) is never re-created by a tab switch.
 *
 * Routing and focus go through the LayerStack (ADR-003): a single `panel`-kind layer — the "active-panel sentinel" — exists exactly while the active session has a panel.
 * The sentinel is a throwaway element, so the stack removing it on tab-switch costs nothing; the persistent content it represents stays mounted.
 * The sentinel sits at the bottom of the stack (below any page/modal), is transparent to unhandled keys (Ctrl+←/→ and friends fall through), and takes focus off the terminal only when it is the top layer.
 */

import type { LayerStack } from './layers/LayerStack';
import type { DismissDecision, LayerHandle } from './layers/types';

interface PanelEntry {
  /** The `.overlay-panel` wrapper holding the caller's content; mounted for the panel's whole lifetime. */
  element: HTMLElement;
  /** The response to a user q/Escape while this panel is the active (top) one. `close` tears the panel down. */
  onUserDismiss: () => DismissDecision;
}

export interface SessionPanelsDeps {
  layers: LayerStack;
  /** The main-panel region; panel content and the sentinel mount here. */
  mainPanelEl: HTMLElement;
  /** The currently active session, resolved live so it can never go stale. */
  getActiveSessionId: () => number | null;
  /** Re-render the tab bar after a panel appears or disappears (drives the per-tab indicator). */
  onChange: () => void;
}

export class SessionPanels {
  private readonly panels = new Map<number, PanelEntry>();
  private sentinel: HTMLElement | null = null;
  private layerHandle: LayerHandle | null = null;

  constructor(private readonly deps: SessionPanelsDeps) {}

  /** Show `content` as the panel for `sessionId`, replacing any existing panel on that session. */
  show(sessionId: number, content: HTMLElement, onUserDismiss: () => DismissDecision): void {
    this.dismiss(sessionId);

    const element = document.createElement('div');
    element.className = 'overlay-panel';
    element.appendChild(content);
    this.deps.mainPanelEl.appendChild(element);

    this.panels.set(sessionId, { element, onUserDismiss });
    this.reconcile();
    this.deps.onChange();
  }

  /** Tear down a session's panel (backend close, tab teardown). Restores focus only if it was the active panel — a hidden panel's removal must not steal focus. */
  dismiss(sessionId: number): void {
    const entry = this.panels.get(sessionId);
    if (!entry) return;
    entry.element.remove();
    this.panels.delete(sessionId);
    this.reconcile();
    this.deps.onChange();
  }

  /** React to a session activation: swap the sentinel to the newly active panel and refocus it when it is on top. */
  syncActive(): void {
    this.reconcile();
    if (this.layerHandle && this.deps.layers.top() === this.layerHandle) {
      this.sentinel?.focus();
    }
  }

  has(sessionId: number): boolean {
    return this.panels.has(sessionId);
  }

  hasAnyVisible(): boolean {
    for (const entry of this.panels.values()) {
      if (entry.element.style.display !== 'none') return true;
    }
    return false;
  }

  /** Show the active session's panel and hide the rest, and keep exactly one sentinel layer iff the active session has a panel. */
  private reconcile(): void {
    const activeId = this.deps.getActiveSessionId();
    for (const [id, entry] of this.panels) {
      entry.element.style.display = id === activeId ? 'block' : 'none';
    }

    const activeHasPanel = activeId !== null && this.panels.has(activeId);
    if (activeHasPanel && !this.layerHandle) {
      this.pushSentinel();
    } else if (!activeHasPanel && this.layerHandle) {
      this.deps.layers.remove(this.layerHandle);
    }
  }

  private pushSentinel(): void {
    const sentinel = document.createElement('div');
    sentinel.className = 'session-panel-sentinel';
    this.deps.mainPanelEl.appendChild(sentinel);
    this.sentinel = sentinel;

    this.layerHandle = this.deps.layers.push({
      name: 'session-panel',
      kind: 'panel',
      element: sentinel,
      onUserDismissRequest: () => this.dismissActivePanel(),
      onCleanup: () => {
        // Fires on both tab-switch removal and dismissal — either way the sentinel is gone. Content bookkeeping lives in show()/dismiss(), not here.
        this.sentinel = null;
        this.layerHandle = null;
      },
    });
  }

  /** A user q/Escape reached the sentinel: route it to the active panel's own decision. On `close` we tear the panel down here (the sentinel represents it but is not it), so tell the stack we already handled it. */
  private dismissActivePanel(): DismissDecision {
    const activeId = this.deps.getActiveSessionId();
    const entry = activeId === null ? undefined : this.panels.get(activeId);
    if (!entry) return 'close';
    const decision = entry.onUserDismiss();
    if (decision === 'close') {
      this.dismiss(activeId!);
      return 'handled';
    }
    return decision;
  }
}
