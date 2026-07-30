import { LayerStack, LayerStackDeps } from './LayerStack';

/**
 * The one sanctioned accessor for the *window* stack (mirroring ADR-002's single error path).
 * Deep call sites — `showModalDialog`, `DropdownMenu.open`, and the like — cannot be threaded a stack instance, so they reach it through `getWindowLayerStack()`.
 *
 * Tab stacks are deliberately not reachable this way: a tab stack is owned by its `SessionTab` and is passed explicitly.
 * Modals and dropdowns are window-level because that is what they are today — every dialog is raised over a hub, every dropdown is anchored inside one.
 * A tab-bound surface that needs to raise a dialog should be given its stack explicitly rather than growing an implicit "current stack" here.
 */

let instance: LayerStack | null = null;

/** Create the singleton window stack. Called once from `main.ts` at startup. */
export function initWindowLayerStack(deps: LayerStackDeps): LayerStack {
  instance = new LayerStack(deps);
  return instance;
}

/** The singleton window stack. Throws if `initWindowLayerStack` has not run yet. */
export function getWindowLayerStack(): LayerStack {
  if (!instance) {
    throw new Error('Window LayerStack not initialized: call initWindowLayerStack() first');
  }
  return instance;
}
