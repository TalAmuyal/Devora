/**
 * Shared types for the layer system (ADR-003).
 *
 * A layer is a stacked UI surface.
 * It declares its behavior through the properties below.
 * How far it extends is decided by *which stack it is in* (the window stack covers the app, a tab stack covers its session), and how it routes and focuses is decided by *its position* in that stack.
 *
 * Authoring uses the presets in `./presets` (`*Layer`) rather than setting these flags by hand.
 */

/**
 * A layer's response to a dismissal request (`q` or `Escape`):
 * - `close`   — pop this layer.
 * - `handled` — the layer did something internal instead (closed a cheatsheet, replaced itself); the key is consumed without popping.
 * - `veto`    — refuse the dismissal (e.g. the zero-profile lock); the key is still consumed.
 */
export type DismissDecision = 'close' | 'handled' | 'veto';

/**
 * What a stack did with a key:
 * - `consumed` — a layer acted on it; the caller must consume the event.
 * - `blocked`  — an opaque layer ended the walk without acting; routing stops, but the event is left alone so it can reach the focused element (this is how a terminal layer receives typing).
 * - `passed`   — no layer claimed it; the caller may route it onward.
 */
export type RouteResult = 'consumed' | 'blocked' | 'passed';

/** Anything that can take keyboard focus, e.g. a terminal pane or a search input. */
export interface Focusable {
  focus(): void;
}

export interface LayerSpec {
  /** Stable identity, e.g. 'ws-hub', 'settings-hub', etc. */
  name: string;

  /** The content element. When `callerMounted`, it is already in the DOM and serves as its own wrapper. */
  element: HTMLElement;

  /**
   * Ends the key walk: keys this layer does not handle go no further down the stack (and no further to the stack below).
   * A transparent layer (an anchored popup, a non-modal floating panel) passes them on.
   * Defaults to `true`.
   */
  opaque?: boolean;

  /** Takes keyboard focus when it becomes the top layer. Defaults to `true` */
  takesFocus?: boolean;

  /** `q`/`Escape` may dismiss it. Defaults to `true`. */
  dismissible?: boolean;

  /** Tab cycles focus within this layer while it is on top. Defaults to `false`. */
  trapsTab?: boolean;

  /**
   * Removing this layer from the top runs the revealed layer's `onReveal`.
   * True for navigation (returning from Settings refreshes the hub), false for a transient sub-interaction such as a dialog, whose caller already refreshes on confirm.
   * Defaults to `false`.
   */
  refreshesRevealedLayer?: boolean;

  /** The caller already placed `element` in the DOM and owns its position; the stack mounts no wrapper. Defaults to `false`. */
  callerMounted?: boolean;

  /** Layer-specific keys, tried first in the walk. Returning `true` consumes the key. */
  onKey?: (e: KeyboardEvent) => boolean;

  /** Response to a user dismissal request. Defaults to `close` when absent. */
  onUserDismissRequest?: () => DismissDecision;

  /** Runs exactly once when the layer is popped, removed, or replaced. */
  onCleanup?: () => void;

  /** Runs when this layer becomes top again after a `refreshesRevealedLayer` layer above it is removed. */
  onReveal?: () => void;

  /** The focus target, resolved at push and at each reveal. */
  resolveFocus?: () => Focusable | null;

  /** Extra classes for the stack-owned wrapper, e.g. `layer-page`. Ignored when `callerMounted`. */
  wrapperClass?: string;
}

export interface LayerHandle {
  readonly name: string;
  /** The content element passed in the spec. */
  readonly element: HTMLElement;
  /** The stack-owned wrapper (identical to `element` when `callerMounted`). */
  readonly wrapper: HTMLElement;
}

export function isOpaque(spec: LayerSpec): boolean {
  return spec.opaque ?? true;
}

export function takesFocus(spec: LayerSpec): boolean {
  return spec.takesFocus ?? true;
}

export function isDismissible(spec: LayerSpec): boolean {
  return spec.dismissible ?? true;
}
