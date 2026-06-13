/** Centered confirmation modal over a semi-transparent backdrop. Resolves `true` on confirm, `false` on cancel/dismiss. DOM: `div.confirmation-dialog-backdrop > div.confirmation-dialog`. */

export interface ConfirmationDialogOptions {
  title: string;
  body: string | HTMLElement;
  confirmLabel: string;
  cancelLabel?: string;
  /** Render only the confirm button (for pure notices); Escape still resolves `false`. */
  hideCancel?: boolean;
}

/**
 * Show a confirmation dialog and return a promise that resolves to `true`
 * (confirmed) or `false` (cancelled). The dialog is appended to
 * `document.body` and removed after resolution.
 *
 * Dismiss actions that resolve `false`: cancel button, backdrop click, Escape key.
 * Confirm actions that resolve `true`: confirm button, Enter key.
 */
export function showConfirmationDialog(options: ConfirmationDialogOptions): Promise<boolean> {
  return new Promise((resolve) => {
    let resolved = false;

    const finish = (value: boolean): void => {
      if (resolved) return;
      resolved = true;
      document.removeEventListener('keydown', onKeydown, true);
      backdrop.remove();
      resolve(value);
    };

    // -- Backdrop --
    const backdrop = document.createElement('div');
    backdrop.className = 'confirmation-dialog-backdrop';
    backdrop.addEventListener('click', (e) => {
      if (e.target === backdrop) {
        finish(false);
      }
    });

    // -- Dialog container --
    const dialog = document.createElement('div');
    dialog.className = 'confirmation-dialog';

    // -- Title --
    const titleEl = document.createElement('div');
    titleEl.className = 'confirmation-dialog-title';
    titleEl.textContent = options.title;
    dialog.appendChild(titleEl);

    // -- Body --
    const bodyEl = document.createElement('div');
    bodyEl.className = 'confirmation-dialog-body';
    if (typeof options.body === 'string') {
      bodyEl.textContent = options.body;
    } else {
      bodyEl.appendChild(options.body);
    }
    dialog.appendChild(bodyEl);

    // -- Button row --
    const actions = document.createElement('div');
    actions.className = 'confirmation-dialog-actions';

    if (!options.hideCancel) {
      const cancelBtn = document.createElement('button');
      cancelBtn.className = 'confirmation-dialog-cancel';
      cancelBtn.textContent = options.cancelLabel ?? 'Cancel';
      cancelBtn.addEventListener('click', () => finish(false));
      actions.appendChild(cancelBtn);
    }

    const confirmBtn = document.createElement('button');
    confirmBtn.className = 'confirmation-dialog-confirm';
    confirmBtn.textContent = options.confirmLabel;
    confirmBtn.addEventListener('click', () => finish(true));
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
        finish(true);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        finish(false);
      }
    };
    document.addEventListener('keydown', onKeydown, true);

    document.body.appendChild(backdrop);
  });
}
