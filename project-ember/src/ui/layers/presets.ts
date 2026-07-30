/**
 * The authoring presets for layers (ADR-003).
 *
 * `LayerStack` knows only the properties in `./types`; these factories bundle them into the four combinations the app actually uses, so a call site says `pageLayer({...})` instead of restating four booleans and hoping they stay consistent across surfaces.
 *
 * A preset says nothing about how far the surface extends — that comes from the stack it is pushed onto.
 * The same `pageLayer` covers the whole window on the window stack and only the session region on a tab stack.
 */

import type { LayerSpec } from './types';

/** The parts a caller supplies; the preset fills in the behavioral flags. */
type LayerOptions = Omit<
  LayerSpec,
  'opaque' | 'takesFocus' | 'dismissible' | 'trapsTab' | 'refreshesRevealedLayer' | 'callerMounted'
>;

/**
 * A full surface that replaces its stack's region: hubs, the User Guide, the Command Palette, etc.
 * Opening one is navigation, so dismissing it refreshes whatever it covered.
 */
export function pageLayer(options: LayerOptions): LayerSpec {
  return {
    ...options,
    refreshesRevealedLayer: true,
    wrapperClass: classes('layer-page', options.wrapperClass),
  };
}

/**
 * A dialog over a dimmed backdrop, with the surface below kept visible as context.
 * It traps Tab, and it is a transient sub-interaction rather than navigation: cancelling must not refresh the caller, which already refreshes on confirm.
 */
export function modalLayer(options: LayerOptions): LayerSpec {
  return {
    ...options,
    trapsTab: true,
    wrapperClass: classes(`layer-modal ${options.name}-backdrop`, options.wrapperClass),
  };
}

/**
 * An anchored light-dismiss surface (a dropdown menu), positioned and mounted by its caller inside the surface it belongs to.
 * It never takes focus and passes on keys it does not handle, so the surface beneath keeps working while it is open.
 */
export function popupLayer(options: LayerOptions): LayerSpec {
  return {
    ...options,
    opaque: false,
    takesFocus: false,
    callerMounted: true,
  };
}

/**
 * A terminal surface: the bottom layer of every tab stack, and the shape a future "run a CLI app over the shell" surface takes.
 *
 * Two contracts make terminal input work, both load-bearing:
 * - It is opaque but consumes nothing, so the walk ends here and the event reaches xterm's hidden textarea.
 * - It is not dismissible, because that textarea only counts as "editable focus" while it actually holds focus — without this, a `q` pressed while focus sat on chrome would tear the terminal down.
 *   It is also the right semantics for a CLI surface: `q` should quit `top`, and the process exiting is what removes the layer.
 */
export function terminalLayer(options: LayerOptions): LayerSpec {
  return {
    ...options,
    dismissible: false,
    wrapperClass: classes('layer-terminal', options.wrapperClass),
  };
}

function classes(preset: string, extra: string | undefined): string {
  return extra ? `${preset} ${extra}` : preset;
}
