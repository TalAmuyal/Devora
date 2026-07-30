import { isEditableElementFocused } from '../focus';
import { cycleFocus } from './tabTrap';
import { isDismissible, isOpaque, takesFocus } from './types';
import type { LayerHandle, LayerSpec, RouteResult } from './types';

export interface LayerStackDeps {
  /** The single element every stack-owned wrapper mounts into. Paint order follows append order, so it must be one host per stack. */
  host: HTMLElement;
  /** Runs when the last layer is removed — the window stack uses it to hand focus back to the active tab. */
  onEmptied?: () => void;
}

interface LayerEntry {
  spec: LayerSpec;
  wrapper: HTMLElement;
  handle: LayerHandle;
}

/**
 * An ordered stack of UI surfaces that owns their mounting, focus, and key routing — see `./CLAUDE.md` for the cross-file reference.
 *
 * The stack is a mechanism with no domain knowledge and no keyboard listener of its own: `LayerRouter` owns the single listener and calls `route`.
 * There is one stack per tab plus one for the window, which is what decides how far a surface extends.
 */
export class LayerStack {
  private readonly deps: LayerStackDeps;
  /** Bottom-to-top; `layers[length - 1]` is the top. */
  private readonly layers: LayerEntry[] = [];

  constructor(deps: LayerStackDeps) {
    this.deps = deps;
  }

  /** Push a layer on top, mounting it and giving it focus unless it declines. */
  push(spec: LayerSpec): LayerHandle {
    const wrapper = this.mountWrapper(spec);
    const entry: LayerEntry = { spec, wrapper, handle: makeHandle(spec, wrapper) };
    this.layers.push(entry);
    if (takesFocus(spec)) {
      this.focusLayer(entry);
    }
    return entry.handle;
  }

  /** Pop the top layer, revealing and focusing whatever is beneath. */
  pop(): void {
    const top = this.topEntry();
    if (top) {
      this.remove(top.handle);
    }
  }

  /**
   * Remove a layer at any position.
   * First removes any caller-mounted layer anchored inside it (a dropdown auto-closes when its host page goes away).
   * Reveals and refocuses only when the removed layer was the top.
   */
  remove(handle: LayerHandle): void {
    const entry = this.entryOf(handle);
    if (!entry) return;
    this.removeContainedLayers(entry);
    const wasTop = this.topEntry() === entry;
    const removedSpec = entry.spec;
    this.detach(entry);
    if (wasTop) {
      this.revealAfterTopRemoval(removedSpec);
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

  /** Focus the top layer (tab activation re-focuses whatever that tab was showing). */
  focusTop(): void {
    const top = this.topEntry();
    if (top) this.focusLayer(top);
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

  /**
   * Whether any layer here owns the keyboard.
   * This is the precondition for the app-wide shortcuts that open or switch surfaces — see `../KeyboardShortcuts.ts`.
   */
  hasOpaqueLayer(): boolean {
    return this.layers.some((l) => isOpaque(l.spec));
  }

  // --- Key routing ---

  /** Walk the stack top-down and report what happened. Never touches the event; `LayerRouter` consumes on `consumed`. */
  route(e: KeyboardEvent): RouteResult {
    const top = this.topEntry();
    if (!top) return 'passed';

    if (e.key === 'Tab' && top.spec.trapsTab === true) {
      cycleFocus(top.wrapper, e.shiftKey);
      return 'consumed';
    }

    for (let i = this.layers.length - 1; i >= 0; i--) {
      const layer = this.layers[i];
      if (layer.spec.onKey?.(e) === true) {
        return 'consumed';
      }
      if (isDismissible(layer.spec) && isDismissKey(e)) {
        // A focused input inside this layer keeps the key: `q` types, Escape reaches the input's own handler.
        if (this.editableFocusedWithin(layer)) return 'blocked';
        this.dismissLayer(layer);
        return 'consumed';
      }
      if (isOpaque(layer.spec)) {
        return 'blocked';
      }
    }
    return 'passed';
  }

  // --- Internals ---

  private topEntry(): LayerEntry | undefined {
    return this.layers[this.layers.length - 1];
  }

  private entryOf(handle: LayerHandle): LayerEntry | undefined {
    return this.layers.find((l) => l.handle === handle);
  }

  /**
   * Mount the layer and return the element the stack treats as its wrapper.
   * Stack-owned wrappers are appended to the one host, so DOM order matches stack order and paint order follows it without any z-index.
   */
  private mountWrapper(spec: LayerSpec): HTMLElement {
    let wrapper: HTMLElement;
    if (spec.callerMounted) {
      wrapper = spec.element;
    } else {
      wrapper = document.createElement('div');
      wrapper.className = 'layer-wrapper';
      if (spec.wrapperClass) {
        wrapper.classList.add(...spec.wrapperClass.split(' '));
      }
      wrapper.appendChild(spec.element);
      this.deps.host.appendChild(wrapper);
    }
    // Focusable out of the tab order so the wrapper can hold keyboard focus as a fallback target.
    wrapper.tabIndex = -1;
    return wrapper;
  }

  private focusLayer(entry: LayerEntry): void {
    (entry.spec.resolveFocus?.() ?? entry.wrapper).focus();
  }

  /** Remove any caller-mounted layer whose element is DOM-contained in `entry`; cleanup and detach only, no reveal. */
  private removeContainedLayers(entry: LayerEntry): void {
    const contained = this.layers.filter(
      (l) => l !== entry && l.spec.callerMounted === true && entry.wrapper.contains(l.wrapper),
    );
    for (const layer of contained) {
      this.detach(layer);
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
   * After removing the top layer: hand focus on when the removed layer had it, otherwise leave the UI untouched.
   * A layer that never took focus (an anchored popup) also never covered anything, so closing it must not move focus or refresh anyone.
   * `onReveal` fires only for a layer that declared itself a navigation step: returning from Settings reloads the hub, while cancelling a dialog must not re-run a reload that could race the caller's own.
   */
  private revealAfterTopRemoval(removedSpec: LayerSpec): void {
    if (!takesFocus(removedSpec)) return;
    const revealed = this.topEntry();
    if (!revealed) {
      this.deps.onEmptied?.();
      return;
    }
    if (removedSpec.refreshesRevealedLayer === true) {
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

  private editableFocusedWithin(layer: LayerEntry): boolean {
    return isEditableElementFocused() && layer.wrapper.contains(document.activeElement);
  }
}

/** `q` and Escape, the vim-style dismissal pair. Modifier combinations are shortcuts, never dismissals. */
function isDismissKey(e: KeyboardEvent): boolean {
  if (e.ctrlKey || e.metaKey || e.altKey) return false;
  return e.key === 'Escape' || e.key === 'q';
}

function makeHandle(spec: LayerSpec, wrapper: HTMLElement): LayerHandle {
  return { name: spec.name, element: spec.element, wrapper };
}
