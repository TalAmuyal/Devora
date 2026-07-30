/** Centered single-text-input modal over a semi-transparent backdrop. Resolves the trimmed input value on confirm, `null` on cancel/dismiss. DOM: `div.text-input-dialog-backdrop > div.text-input-dialog`. */

import { showModalDialog, ModalDialogHandle } from './ModalDialog';

export interface TextInputDialogOptions {
  title: string;
  initialValue: string;
  confirmLabel: string;
  cancelLabel?: string;
}

/**
 * Show a text-input dialog and return a promise that resolves to the trimmed input value (confirmed) or `null` (cancelled/dismissed).
 *
 * The input is pre-filled with `initialValue` and fully selected, so typing replaces it.
 * Confirming an empty (all-whitespace) value is a no-op — the dialog stays open.
 *
 * The dialog is a `modalLayer` (see `./ModalDialog`): Escape and a backdrop click resolve `null`; Enter and the confirm button resolve the value; `q` types into the focused input.
 */
export function showTextInputDialog(options: TextInputDialogOptions): Promise<string | null> {
  return new Promise((resolve) => {
    let result: string | null = null;
    let handle: ModalDialogHandle;

    const dialog = document.createElement('div');
    dialog.className = 'text-input-dialog';

    const titleEl = document.createElement('div');
    titleEl.className = 'text-input-dialog-title';
    titleEl.textContent = options.title;
    dialog.appendChild(titleEl);

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'text-input-dialog-input';
    input.value = options.initialValue;
    dialog.appendChild(input);

    const confirm = (): void => {
      const value = input.value.trim();
      if (value === '') return;
      result = value;
      handle.close();
    };

    const actions = document.createElement('div');
    actions.className = 'text-input-dialog-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'text-input-dialog-cancel';
    cancelBtn.textContent = options.cancelLabel ?? 'Cancel';
    cancelBtn.addEventListener('click', () => handle.close());
    actions.appendChild(cancelBtn);

    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'text-input-dialog-confirm';
    confirmBtn.textContent = options.confirmLabel;
    confirmBtn.addEventListener('click', confirm);
    actions.appendChild(confirmBtn);

    dialog.appendChild(actions);

    handle = showModalDialog({
      name: 'text-input-dialog',
      element: dialog,
      onDismiss: () => handle.close(),
      onConfirm: confirm,
      onCleanup: () => resolve(result),
      // Focus and select the pre-fill so typing replaces it (the LayerStack only calls focus()).
      resolveFocus: () => ({
        focus: () => {
          input.focus();
          input.select();
        },
      }),
    });
  });
}
