/** Shared types for the layer system (ADR-003). A layer is a stacked UI surface — a full page, a modal dialog, an anchored popup, or a per-session panel — routed and focused by its position in the LayerStack rather than by registration order or incidental focus. */

/**
 * A layer's behavioral class — modality, focus ownership, and key routing — not its visual size.
 *
 * - `page`  — full-screen surface (hubs, User Guide, Command Palette); opaque modality barrier.
 * - `modal` — dialog over a backdrop; opaque barrier with a Tab focus trap.
 * - `popup` — anchored light-dismiss surface (dropdowns); never moves focus, transparent to keys it does not handle.
 * - `panel` — per-session region over the terminal (Crit, task-creation); inserted at the bottom, transparent to unhandled keys.
 */
export type LayerKind = 'page' | 'modal' | 'popup' | 'panel';

/**
 * A layer's response to a dismissal request (`q`, `Escape`, or `Ctrl+S` on a page):
 * - `close`   — pop this layer.
 * - `handled` — the layer did something internal instead (closed a cheatsheet, replaced itself); the key is consumed without popping.
 * - `veto`    — refuse the dismissal (e.g. the zero-profile lock); the key is still consumed.
 */
export type DismissDecision = 'close' | 'handled' | 'veto';

/** Anything that can take keyboard focus, e.g. a terminal pane or a search input. */
export interface Focusable {
  focus(): void;
}

export interface LayerSpec {
  /** Stable identity, e.g. 'ws-hub', 'settings-hub', 'confirmation-dialog'. Also seeds a modal's `${name}-backdrop` class. */
  name: string;
  kind: LayerKind;
  /** The content element. For `popup`/`panel` the caller has already placed it in the DOM; the stack uses it as its own wrapper. */
  element: HTMLElement;
  /** Layer-specific keys, tried first in the walk. Returning `true` consumes the key. */
  onKey?: (e: KeyboardEvent) => boolean;
  /** Response to a user dismissal request. Defaults to `close` when absent. */
  onUserDismissRequest?: () => DismissDecision;
  /** Runs exactly once when the layer is popped, removed, or replaced. */
  onCleanup?: () => void;
  /** Runs when this layer becomes top again after a covering layer pops (refresh-on-reveal). */
  onReveal?: () => void;
  /** The focus target, resolved at push and at each reveal. */
  resolveFocus?: () => Focusable | null;
  /** Extra class for the stack-owned wrapper, e.g. 'overlay-passthrough' for the palette. */
  wrapperClass?: string;
}

export interface LayerHandle {
  readonly name: string;
  readonly kind: LayerKind;
  /** The content element passed in the spec. */
  readonly element: HTMLElement;
  /** The stack-owned wrapper (identical to `element` for `popup`/`panel`). */
  readonly wrapper: HTMLElement;
}
