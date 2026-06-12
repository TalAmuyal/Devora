/** Centered single-text-input modal over a semi-transparent backdrop. Resolves the trimmed input value on confirm, `null` on cancel/dismiss. DOM: `div.text-input-dialog-backdrop > div.text-input-dialog`. */

export interface TextInputDialogOptions {
  title: string;
  initialValue: string;
  confirmLabel: string;
  cancelLabel?: string;
}

/**
 * Show a text-input dialog and return a promise that resolves to the trimmed input value (confirmed) or `null` (cancelled).
 * The dialog is appended to `document.body` and removed after resolution.
 *
 * The input is pre-filled with `initialValue` and fully selected, so typing replaces it.
 * Confirming an empty (all-whitespace) value is a no-op — the dialog stays open.
 *
 * Dismiss actions that resolve `null`: cancel button, backdrop click, Escape key.
 * Confirm actions that resolve the value: confirm button, Enter key.
 */
export function showTextInputDialog(options: TextInputDialogOptions): Promise<string | null> {
  return new Promise((resolve) => {
    let resolved = false;

    const finish = (value: string | null): void => {
      if (resolved) return;
      resolved = true;
      document.removeEventListener('keydown', onKeydown, true);
      backdrop.remove();
      resolve(value);
    };

    const confirm = (): void => {
      const value = input.value.trim();
      if (value === '') return;
      finish(value);
    };

    // -- Backdrop --
    const backdrop = document.createElement('div');
    backdrop.className = 'text-input-dialog-backdrop';
    backdrop.addEventListener('click', (e) => {
      if (e.target === backdrop) {
        finish(null);
      }
    });

    // -- Dialog container --
    const dialog = document.createElement('div');
    dialog.className = 'text-input-dialog';

    // -- Title --
    const titleEl = document.createElement('div');
    titleEl.className = 'text-input-dialog-title';
    titleEl.textContent = options.title;
    dialog.appendChild(titleEl);

    // -- Input --
    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'text-input-dialog-input';
    input.value = options.initialValue;
    dialog.appendChild(input);

    // -- Button row --
    const actions = document.createElement('div');
    actions.className = 'text-input-dialog-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'text-input-dialog-cancel';
    cancelBtn.textContent = options.cancelLabel ?? 'Cancel';
    cancelBtn.addEventListener('click', () => finish(null));
    actions.appendChild(cancelBtn);

    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'text-input-dialog-confirm';
    confirmBtn.textContent = options.confirmLabel;
    confirmBtn.addEventListener('click', confirm);
    actions.appendChild(confirmBtn);

    dialog.appendChild(actions);
    backdrop.appendChild(dialog);

    // -- Focus trapping: capture all keyboard events on the backdrop --
    backdrop.addEventListener('keydown', (e) => {
      e.stopPropagation();
    });

    // -- Global keyboard shortcuts (capture phase so they fire before anything else) --
    const onKeydown = (e: KeyboardEvent): void => {
      if (e.key === 'Enter') {
        e.preventDefault();
        e.stopPropagation();
        confirm();
      } else if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        finish(null);
      }
    };
    document.addEventListener('keydown', onKeydown, true);

    document.body.appendChild(backdrop);

    input.focus();
    input.select();
  });
}
