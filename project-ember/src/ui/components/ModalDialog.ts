/**
 * The shared primitive for modal dialogs: mounts `element` as a `modal` layer on the LayerStack and routes its keys and backdrop click.
 * The stack owns the wrapper (class `layer-modal ${name}-backdrop`, over a backdrop on `document.body`), the Tab focus trap, and focus restoration to the layer beneath on close.
 *
 * A modal owns Escape and Enter here — `onKey` runs before the central editable guard, so a single press cancels/confirms regardless of what holds focus.
 * `q` is intentionally left to the guard: it types inside a focused input and dismisses only when a non-editable element (e.g. a button) holds focus.
 * See `../layers/CLAUDE.md` ("Modals and pages treat Escape oppositely") for why pages take the opposite approach.
 */

import { getLayerStack } from '../layers/stack';
import type { Focusable } from '../layers/types';

export interface ModalDialogSpec {
  /** Stable identity; also seeds the wrapper's `${name}-backdrop` class (which carries the backdrop CSS). */
  name: string;
  /** The dialog content element, placed inside the stack-owned backdrop wrapper. */
  element: HTMLElement;
  /** Escape, `q` on a non-editable focus, and (by default) a backdrop click route here — the dialog's cancel/dismiss. */
  onDismiss: () => void;
  /** Enter routes here — the dialog's confirm. */
  onConfirm?: () => void;
  /** A backdrop click routes here when set, otherwise to `onDismiss` (lets a two-phase dialog make a backdrop click inert during progress). */
  onBackdropClick?: () => void;
  /** Runs exactly once when the layer is removed — the promise dialogs resolve here so any teardown path settles them. */
  onCleanup?: () => void;
  /** Initial focus target, focused by the stack on push (and restored on Tab-trap recovery). */
  resolveFocus?: () => Focusable | null;
}

export interface ModalDialogHandle {
  /** Remove the modal layer (idempotent). */
  close(): void;
}

export function showModalDialog(spec: ModalDialogSpec): ModalDialogHandle {
  const layers = getLayerStack();
  const handle = layers.push({
    name: spec.name,
    kind: 'modal',
    element: spec.element,
    onKey: (e) => {
      if (e.key === 'Escape') {
        spec.onDismiss();
        return true;
      }
      if (e.key === 'Enter') {
        spec.onConfirm?.();
        return true;
      }
      return false;
    },
    // `q` on a non-editable focus reaches here (Escape is already handled in onKey): dismiss.
    onUserDismissRequest: () => {
      spec.onDismiss();
      return 'handled';
    },
    onCleanup: spec.onCleanup,
    resolveFocus: spec.resolveFocus,
  });

  const onBackdrop = spec.onBackdropClick ?? spec.onDismiss;
  handle.wrapper.addEventListener('click', (e) => {
    if (e.target === handle.wrapper) onBackdrop();
  });

  return { close: () => layers.remove(handle) };
}
