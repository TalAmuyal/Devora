import { isEditableElementFocused } from '../focus';
import { cycleFocus } from './tabTrap';
import type { Focusable, LayerHandle, LayerKind, LayerSpec } from './types';

export interface LayerStackDeps {
  /** Host for `page` wrappers (production: `#app`). */
  pageHost: HTMLElement;
  /** Host for `modal` wrappers (production: `document.body`). */
  modalHost: HTMLElement;
  /** Focus target when the stack empties, resolved at that moment (production: the active session's terminal pane). */
  resolveBaseFocus: () => Focusable | null;
}

interface LayerEntry {
  spec: LayerSpec;
  wrapper: HTMLElement;
  handle: LayerHandle;
}

/**
 * The single owner of layer keyboard routing and focus (ADR-003).
 *
 * One `window`-capture keydown listener walks the stack top-to-bottom; `page` and `modal` layers are opaque barriers while `popup` and `panel` layers pass unhandled keys through.
 * Focus is resolved from stack position at every transition, never from a value captured earlier.
 */
export class LayerStack {
  private readonly deps: LayerStackDeps;
  /** Bottom-to-top; `layers[length - 1]` is the top. */
  private readonly layers: LayerEntry[] = [];
  private keyObserver: ((e: KeyboardEvent) => void) | null = null;
  private globalHandler: ((e: KeyboardEvent) => boolean) | null = null;
  private pageBarrierAdmits: ((e: KeyboardEvent) => boolean) | null = null;

  constructor(deps: LayerStackDeps) {
    this.deps = deps;
  }

  /** Register THE window-capture keydown listener. Call once, before any other keydown handler. Idempotent per instance. */
  install(): void {
    window.addEventListener('keydown', this.boundHandleKeydown, true);
  }

  uninstall(): void {
    window.removeEventListener('keydown', this.boundHandleKeydown, true);
  }

  /** A non-consuming observer that sees every keydown (Shift-Shift tracking). */
  setKeyObserver(fn: (e: KeyboardEvent) => void): void {
    this.keyObserver = fn;
  }

  /** The app-wide shortcuts, consulted when a key reaches the base (empty stack or passes all surfuce barriers). Returning `true` consumes. */
  setGlobalHandler(fn: (e: KeyboardEvent) => boolean): void {
    this.globalHandler = fn;
  }

  /** The predicate for which base shortcuts survive a `page` barrier — owned by KeyboardShortcuts (the single source of truth for app-wide keys). */
  setPageBarrierAdmits(fn: (e: KeyboardEvent) => boolean): void {
    this.pageBarrierAdmits = fn;
  }

  /** Push a layer: `panel` inserts at the bottom (it must never intercept a page/modal above it), every other kind goes on top. */
  push(spec: LayerSpec): LayerHandle {
    const wrapper = this.mountWrapper(spec);
    const entry: LayerEntry = { spec, wrapper, handle: makeHandle(spec, wrapper) };
    if (spec.kind === 'panel') {
      this.layers.unshift(entry);
    } else {
      this.layers.push(entry);
    }
    // Focus the layer only when it landed on top; `popup` never moves focus.
    if (this.topEntry() === entry && spec.kind !== 'popup') {
      this.focusLayer(entry);
    }
    return entry.handle;
  }

  /** Pop the top layer, revealing and focusing whatever is beneath (or the base when the stack empties). */
  pop(): void {
    const top = this.topEntry();
    if (top) {
      this.remove(top.handle);
    }
  }

  /**
   * Remove a layer at any position.
   * First removes any `popup` anchored inside it (a dropdown auto-closes when its host page goes away).
   * Reveals and refocuses only when the removed layer was the top.
   */
  remove(handle: LayerHandle): void {
    const entry = this.entryOf(handle);
    if (!entry) return;
    this.removeContainedPopups(entry);
    const wasTop = this.topEntry() === entry;
    const removedKind = entry.spec.kind;
    this.detach(entry);
    if (wasTop) {
      this.revealAfterTopRemoval(removedKind);
    }
  }

  /** Atomically swap the top layer: the old layer's cleanup runs, the new layer mounts and takes focus once, and nothing below is revealed. */
  replaceTop(spec: LayerSpec): LayerHandle {
    const old = this.topEntry();
    if (old) {
      this.detach(old);
    }
    return this.push(spec);
  }

  /** Run the top layer's dismissal path, as a user `q`/`Escape` would. Returns whether a layer handled it. */
  requestUserDismiss(): boolean {
    const top = this.topEntry();
    if (!top) return false;
    this.dismissLayer(top);
    return true;
  }

  /** Teardown: cleanup and detach every layer, top-first. No reveal or focus work. */
  clear(): void {
    while (this.layers.length > 0) {
      this.detach(this.layers[this.layers.length - 1]);
    }
  }

  isEmpty(): boolean {
    return this.layers.length === 0;
  }

  depth(): number {
    return this.layers.length;
  }

  top(): LayerHandle | null {
    return this.topEntry()?.handle ?? null;
  }

  find(name: string): LayerHandle | null {
    return this.layers.find((l) => l.spec.name === name)?.handle ?? null;
  }

  topOf(kind: LayerKind): LayerHandle | null {
    for (let i = this.layers.length - 1; i >= 0; i--) {
      if (this.layers[i].spec.kind === kind) return this.layers[i].handle;
    }
    return null;
  }

  // --- Key dispatch ---

  private readonly boundHandleKeydown = (e: KeyboardEvent): void => this.handleKeydown(e);

  private handleKeydown(e: KeyboardEvent): void {
    this.keyObserver?.(e);

    if (this.layers.length === 0) {
      if (this.globalHandler?.(e)) this.consume(e);
      return;
    }

    const top = this.topEntry()!;
    if (e.key === 'Tab' && top.spec.kind === 'modal') {
      cycleFocus(top.wrapper, e.shiftKey);
      this.consume(e);
      return;
    }

    for (let i = this.layers.length - 1; i >= 0; i--) {
      const layer = this.layers[i];
      if (layer.spec.onKey?.(e) === true) {
        this.consume(e);
        return;
      }
      if (this.isDismissKey(e, layer)) {
        // A focused input inside this layer keeps the key: `q` types, Escape reaches the input's own handler.
        if (this.editableFocusedWithin(layer)) return;
        this.dismissLayer(layer);
        this.consume(e);
        return;
      }
      if (layer.spec.kind === 'page' || layer.spec.kind === 'modal') {
        if (this.allowedThroughBarrier(e, layer.spec.kind) && this.globalHandler?.(e)) {
          this.consume(e);
        }
        return; // the barrier ends the walk
      }
      // `popup` and `panel` are transparent to unhandled keys — fall through to the layer below.
    }

    // Only popups/panels were present and none handled the key.
    if (this.globalHandler?.(e)) this.consume(e);
  }

  private isDismissKey(e: KeyboardEvent, layer: LayerEntry): boolean {
    const bareModifier = !e.ctrlKey && !e.metaKey && !e.altKey;
    if (bareModifier && (e.key === 'Escape' || e.key === 'q')) return true;
    // Ctrl+S toggles a page closed, mirroring the pre-layer Workspace Hub shortcut.
    if (
      layer.spec.kind === 'page' &&
      e.ctrlKey &&
      !e.shiftKey &&
      !e.metaKey &&
      !e.altKey &&
      e.code === 'KeyS'
    ) {
      return true;
    }
    return false;
  }

  /**
   * Which keys reach the global handler through a barrier: under a `modal`, nothing; under a `page`, whatever KeyboardShortcuts admits (F1 + font sizing).
   * The page admit-list lives in KeyboardShortcuts (via setPageBarrierAdmits), so the barrier and the shortcuts it gates can never drift.
   */
  private allowedThroughBarrier(e: KeyboardEvent, kind: 'page' | 'modal'): boolean {
    if (kind === 'modal') return false;
    return this.pageBarrierAdmits?.(e) ?? false;
  }

  private editableFocusedWithin(layer: LayerEntry): boolean {
    return isEditableElementFocused() && layer.wrapper.contains(document.activeElement);
  }

  private consume(e: KeyboardEvent): void {
    e.preventDefault();
    // stopImmediatePropagation (not stopPropagation) so no sibling window-capture listener can act on a consumed key.
    e.stopImmediatePropagation();
  }

  // --- Internals ---

  private topEntry(): LayerEntry | undefined {
    return this.layers[this.layers.length - 1];
  }

  private entryOf(handle: LayerHandle): LayerEntry | undefined {
    return this.layers.find((l) => l.handle === handle);
  }

  private mountWrapper(spec: LayerSpec): HTMLElement {
    let wrapper: HTMLElement;
    if (spec.kind === 'popup' || spec.kind === 'panel') {
      // The caller already placed these in the DOM; the element is its own wrapper.
      wrapper = spec.element;
    } else if (spec.kind === 'page') {
      wrapper = document.createElement('div');
      wrapper.className = 'overlay-tab-covering layer-page';
      wrapper.appendChild(spec.element);
      this.deps.pageHost.appendChild(wrapper);
    } else {
      wrapper = document.createElement('div');
      wrapper.className = `layer-modal ${spec.name}-backdrop`;
      wrapper.appendChild(spec.element);
      this.deps.modalHost.appendChild(wrapper);
    }
    if (spec.wrapperClass) {
      wrapper.classList.add(spec.wrapperClass);
    }
    // Focusable out of the tab order so the wrapper can hold keyboard focus as a fallback target.
    wrapper.tabIndex = -1;
    return wrapper;
  }

  private focusLayer(entry: LayerEntry): void {
    (entry.spec.resolveFocus?.() ?? entry.wrapper).focus();
  }

  /** Remove any `popup` layers whose element is DOM-contained in `entry`; cleanup and detach only, no reveal. */
  private removeContainedPopups(entry: LayerEntry): void {
    const contained = this.layers.filter(
      (l) => l !== entry && l.spec.kind === 'popup' && entry.wrapper.contains(l.wrapper),
    );
    for (const popup of contained) {
      this.detach(popup);
    }
  }

  /** Splice the entry out, run its cleanup exactly once, and remove its wrapper from the DOM. A no-op if the entry is already gone. */
  private detach(entry: LayerEntry): void {
    const index = this.layers.indexOf(entry);
    if (index === -1) return;
    this.layers.splice(index, 1);
    try {
      entry.spec.onCleanup?.();
    } catch (e) {
      console.error(`Layer '${entry.spec.name}' cleanup failed:`, e);
    }
    entry.wrapper.remove();
  }

  /**
   * After removing the top layer: focus the base when the stack emptied, otherwise refocus the newly exposed layer — but only when a *covering* layer (page/modal) was removed.
   * A transparent popup/panel never took focus or hid the layer beneath, so closing it leaves that layer undisturbed.
   * `onReveal` fires only when a *page* cover is removed — it is a navigation refresh (returning from Settings/Health/the User Guide).
   * A modal is a transient sub-interaction: on confirm the caller refreshes, on cancel nothing changed, so re-running the revealed page's `onReveal` would be redundant and could race its own reload.
   * Focus is still restored for a modal.
   */
  private revealAfterTopRemoval(removedKind: LayerKind): void {
    const revealed = this.topEntry();
    if (!revealed) {
      this.deps.resolveBaseFocus()?.focus();
      return;
    }
    if (removedKind !== 'page' && removedKind !== 'modal') return;
    if (removedKind === 'page') {
      revealed.spec.onReveal?.();
    }
    this.focusLayer(revealed);
  }

  private dismissLayer(entry: LayerEntry): void {
    const decision = entry.spec.onUserDismissRequest?.() ?? 'close';
    if (decision === 'close') {
      this.remove(entry.handle);
    }
  }
}

function makeHandle(spec: LayerSpec, wrapper: HTMLElement): LayerHandle {
  return { name: spec.name, kind: spec.kind, element: spec.element, wrapper };
}
