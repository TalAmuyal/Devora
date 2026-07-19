/** Centered confirmation modal over a semi-transparent backdrop. Resolves `true` on confirm, `false` on cancel/dismiss. DOM: `div.confirmation-dialog-backdrop > div.confirmation-dialog`. */

import { showModalDialog, ModalDialogHandle } from './ModalDialog';

export interface ConfirmationDialogOptions {
  title: string;
  body: string | HTMLElement;
  confirmLabel: string;
  cancelLabel?: string;
  /** Render only the confirm button (for pure notices); Escape/q still resolve `false`. */
  hideCancel?: boolean;
}

/**
 * Show a confirmation dialog and return a promise that resolves to `true`
 * (confirmed) or `false` (cancelled/dismissed).
 *
 * The dialog is a `modal` layer (ADR-003): Escape, `q` (on a non-editable focus), and a backdrop click resolve `false`; Enter and the confirm button resolve `true`. Focus lands on the confirm button and is restored to the layer beneath on close.
 */
export function showConfirmationDialog(options: ConfirmationDialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    let result = false;
    let handle: ModalDialogHandle;

    const confirm = (): void => {
      result = true;
      handle.close();
    };

    const dialog = document.createElement('div');
    dialog.className = 'confirmation-dialog';

    const titleEl = document.createElement('div');
    titleEl.className = 'confirmation-dialog-title';
    titleEl.textContent = options.title;
    dialog.appendChild(titleEl);

    const bodyEl = document.createElement('div');
    bodyEl.className = 'confirmation-dialog-body';
    if (typeof options.body === 'string') {
      bodyEl.textContent = options.body;
    } else {
      bodyEl.appendChild(options.body);
    }
    dialog.appendChild(bodyEl);

    const actions = document.createElement('div');
    actions.className = 'confirmation-dialog-actions';

    if (!options.hideCancel) {
      const cancelBtn = document.createElement('button');
      cancelBtn.className = 'confirmation-dialog-cancel';
      cancelBtn.textContent = options.cancelLabel ?? 'Cancel';
      cancelBtn.addEventListener('click', () => handle.close());
      actions.appendChild(cancelBtn);
    }

    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'confirmation-dialog-confirm';
    confirmBtn.textContent = options.confirmLabel;
    confirmBtn.addEventListener('click', confirm);
    actions.appendChild(confirmBtn);

    dialog.appendChild(actions);

    handle = showModalDialog({
      name: 'confirmation-dialog',
      element: dialog,
      onDismiss: () => handle.close(),
      onConfirm: confirm,
      onCleanup: () => resolve(result),
      resolveFocus: () => confirmBtn,
    });
  });
}
