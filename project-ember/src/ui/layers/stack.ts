import { LayerStack, LayerStackDeps } from './LayerStack';

/**
 * The one sanctioned LayerStack accessor (mirroring ADR-002's single error path).
 * Deep call sites — `showConfirmationDialog`, `DropdownMenu.open`, and the like — cannot be threaded a stack instance, so they reach it through `getLayerStack()` instead.
 */

let instance: LayerStack | null = null;

/** Create the singleton LayerStack. Called once from `main.ts` at startup. */
export function initLayerStack(deps: LayerStackDeps): LayerStack {
  instance = new LayerStack(deps);
  return instance;
}

/** The singleton LayerStack. Throws if `initLayerStack` has not run yet. */
export function getLayerStack(): LayerStack {
  if (!instance) {
    throw new Error('LayerStack not initialized: call initLayerStack() first');
  }
  return instance;
}
