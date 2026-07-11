import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { showConfirmationDialog } from '../ConfirmationDialog';
import { installModalStack, teardownModalStack, pressKey } from './modalTestHarness';

describe('showConfirmationDialog', () => {
  beforeEach(() => installModalStack());
  afterEach(() => teardownModalStack());

  it('appends a backdrop and dialog to document.body', () => {
    showConfirmationDialog({
      title: 'Delete?',
      body: 'This cannot be undone.',
      confirmLabel: 'Delete',
    });
    expect(document.querySelector('.confirmation-dialog-backdrop')).not.toBeNull();
    expect(document.querySelector('.confirmation-dialog')).not.toBeNull();
  });

  it('renders the title text', () => {
    showConfirmationDialog({
      title: 'Confirm action',
      body: 'Are you sure?',
      confirmLabel: 'Yes',
    });
    const dialog = document.querySelector('.confirmation-dialog')!;
    expect(dialog.textContent).toContain('Confirm action');
  });

  it('renders the body as text when given a string', () => {
    showConfirmationDialog({
      title: 'Title',
      body: 'Body text here',
      confirmLabel: 'OK',
    });
    const dialog = document.querySelector('.confirmation-dialog')!;
    expect(dialog.textContent).toContain('Body text here');
  });

  it('renders the body as an HTMLElement when given one', () => {
    const bodyEl = document.createElement('div');
    bodyEl.className = 'custom-body';
    bodyEl.textContent = 'Custom content';
    showConfirmationDialog({
      title: 'Title',
      body: bodyEl,
      confirmLabel: 'OK',
    });
    const dialog = document.querySelector('.confirmation-dialog')!;
    expect(dialog.querySelector('.custom-body')).not.toBeNull();
    expect(dialog.textContent).toContain('Custom content');
  });

  it('renders confirm and cancel buttons with correct labels', () => {
    showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Delete',
      cancelLabel: 'Nope',
    });
    const confirm = document.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement;
    const cancel = document.querySelector('.confirmation-dialog-cancel') as HTMLButtonElement;
    expect(confirm).not.toBeNull();
    expect(cancel).not.toBeNull();
    expect(confirm.textContent).toBe('Delete');
    expect(cancel.textContent).toBe('Nope');
  });

  it('defaults cancelLabel to "Cancel"', () => {
    showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'OK',
    });
    const cancel = document.querySelector('.confirmation-dialog-cancel') as HTMLButtonElement;
    expect(cancel.textContent).toBe('Cancel');
  });

  it('resolves true when confirm button is clicked', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    const confirm = document.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement;
    confirm.click();
    expect(await promise).toBe(true);
  });

  it('resolves false when cancel button is clicked', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    const cancel = document.querySelector('.confirmation-dialog-cancel') as HTMLButtonElement;
    cancel.click();
    expect(await promise).toBe(false);
  });

  it('resolves false when backdrop is clicked', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    const backdrop = document.querySelector('.confirmation-dialog-backdrop') as HTMLElement;
    backdrop.click();
    expect(await promise).toBe(false);
  });

  it('does not resolve when clicking inside the dialog (not on backdrop)', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    const dialog = document.querySelector('.confirmation-dialog') as HTMLElement;
    dialog.click();

    // Verify the dialog is still present (not resolved yet)
    expect(document.querySelector('.confirmation-dialog')).not.toBeNull();

    // Clean up by confirming
    const confirm = document.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement;
    confirm.click();
    await promise;
  });

  it('resolves true when Enter key is pressed', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    pressKey({ key: 'Enter', code: 'Enter' });
    expect(await promise).toBe(true);
  });

  it('resolves false when Escape key is pressed', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    pressKey({ key: 'Escape', code: 'Escape' });
    expect(await promise).toBe(false);
  });

  it('resolves false when q is pressed (vim-style dismissal)', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    pressKey({ key: 'q', code: 'KeyQ' });
    expect(await promise).toBe(false);
  });

  it('removes the dialog from the DOM after confirmation', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    const confirm = document.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement;
    confirm.click();
    await promise;
    expect(document.querySelector('.confirmation-dialog-backdrop')).toBeNull();
    expect(document.querySelector('.confirmation-dialog')).toBeNull();
  });

  it('removes the dialog from the DOM after cancellation', async () => {
    const promise = showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'Yes',
    });
    const cancel = document.querySelector('.confirmation-dialog-cancel') as HTMLButtonElement;
    cancel.click();
    await promise;
    expect(document.querySelector('.confirmation-dialog-backdrop')).toBeNull();
    expect(document.querySelector('.confirmation-dialog')).toBeNull();
  });

  it('focuses the confirm button on open', () => {
    showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'OK',
    });
    expect(document.activeElement).toBe(document.querySelector('.confirmation-dialog-confirm'));
  });

  it('confirm button has correct CSS class', () => {
    showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'OK',
    });
    const confirm = document.querySelector('.confirmation-dialog-confirm');
    expect(confirm?.tagName).toBe('BUTTON');
  });

  it('cancel button has correct CSS class', () => {
    showConfirmationDialog({
      title: 'Title',
      body: 'Body',
      confirmLabel: 'OK',
    });
    const cancel = document.querySelector('.confirmation-dialog-cancel');
    expect(cancel?.tagName).toBe('BUTTON');
  });

  it('omits the cancel button when hideCancel is set', () => {
    showConfirmationDialog({
      title: 'Cannot delete profile',
      body: 'Close the blocking sessions first.',
      confirmLabel: 'OK',
      hideCancel: true,
    });
    expect(document.querySelector('.confirmation-dialog-cancel')).toBeNull();
    expect(document.querySelector('.confirmation-dialog-confirm')).not.toBeNull();
  });

  it('still resolves false on Escape when hideCancel is set', async () => {
    const promise = showConfirmationDialog({
      title: 'Notice',
      body: 'Body',
      confirmLabel: 'OK',
      hideCancel: true,
    });
    pressKey({ key: 'Escape', code: 'Escape' });
    expect(await promise).toBe(false);
  });
});
