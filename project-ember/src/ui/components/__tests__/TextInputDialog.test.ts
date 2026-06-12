import { describe, it, expect, vi, afterEach } from 'vitest';
import { showTextInputDialog } from '../TextInputDialog';

const getInput = () => document.querySelector('.text-input-dialog-input') as HTMLInputElement;
const getConfirm = () => document.querySelector('.text-input-dialog-confirm') as HTMLButtonElement;
const getCancel = () => document.querySelector('.text-input-dialog-cancel') as HTMLButtonElement;

describe('showTextInputDialog', () => {
  afterEach(() => {
    // Clean up any dialogs left in the DOM
    document.querySelectorAll('.text-input-dialog-backdrop').forEach((el) => el.remove());
  });

  it('appends a backdrop and dialog to document.body', () => {
    showTextInputDialog({
      title: 'Repurpose Workspace',
      initialValue: 'Old task',
      confirmLabel: 'Repurpose',
    });
    expect(document.querySelector('.text-input-dialog-backdrop')).not.toBeNull();
    expect(document.querySelector('.text-input-dialog')).not.toBeNull();
  });

  it('renders the title text', () => {
    showTextInputDialog({
      title: 'Repurpose Workspace',
      initialValue: '',
      confirmLabel: 'OK',
    });
    const dialog = document.querySelector('.text-input-dialog')!;
    expect(dialog.textContent).toContain('Repurpose Workspace');
  });

  it('pre-fills the input with the initial value', () => {
    showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    expect(getInput().value).toBe('Old task');
  });

  it('focuses the input with the pre-fill fully selected', () => {
    showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    const input = getInput();
    expect(document.activeElement).toBe(input);
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe('Old task'.length);
  });

  it('renders confirm and cancel buttons with correct labels', () => {
    showTextInputDialog({
      title: 'Title',
      initialValue: '',
      confirmLabel: 'Repurpose',
      cancelLabel: 'Nope',
    });
    expect(getConfirm().textContent).toBe('Repurpose');
    expect(getCancel().textContent).toBe('Nope');
  });

  it('defaults cancelLabel to "Cancel"', () => {
    showTextInputDialog({
      title: 'Title',
      initialValue: '',
      confirmLabel: 'OK',
    });
    expect(getCancel().textContent).toBe('Cancel');
  });

  it('resolves the input value when confirm button is clicked', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    getInput().value = 'New task';
    getConfirm().click();
    expect(await promise).toBe('New task');
  });

  it('resolves the trimmed input value', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: '',
      confirmLabel: 'OK',
    });
    getInput().value = '  New task  ';
    getConfirm().click();
    expect(await promise).toBe('New task');
  });

  it('resolves the input value when Enter is pressed', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(await promise).toBe('Old task');
  });

  it('keeps the dialog open when confirming an empty value', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: '',
      confirmLabel: 'OK',
    });
    getInput().value = '   ';
    getConfirm().click();
    expect(document.querySelector('.text-input-dialog')).not.toBeNull();

    // Clean up by confirming a real value
    getInput().value = 'done';
    getConfirm().click();
    expect(await promise).toBe('done');
  });

  it('resolves null when cancel button is clicked', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    getCancel().click();
    expect(await promise).toBeNull();
  });

  it('resolves null when Escape is pressed', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(await promise).toBeNull();
  });

  it('resolves null when backdrop is clicked', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    (document.querySelector('.text-input-dialog-backdrop') as HTMLElement).click();
    expect(await promise).toBeNull();
  });

  it('does not resolve when clicking inside the dialog (not on backdrop)', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    (document.querySelector('.text-input-dialog') as HTMLElement).click();
    expect(document.querySelector('.text-input-dialog')).not.toBeNull();

    // Clean up by cancelling
    getCancel().click();
    expect(await promise).toBeNull();
  });

  it('removes the dialog from the DOM after confirmation', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    getConfirm().click();
    await promise;
    expect(document.querySelector('.text-input-dialog-backdrop')).toBeNull();
    expect(document.querySelector('.text-input-dialog')).toBeNull();
  });

  it('removes the dialog from the DOM after cancellation', async () => {
    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    getCancel().click();
    await promise;
    expect(document.querySelector('.text-input-dialog-backdrop')).toBeNull();
    expect(document.querySelector('.text-input-dialog')).toBeNull();
  });

  it('stops keyboard events from propagating beyond the backdrop', async () => {
    const bodyHandler = vi.fn();
    document.body.addEventListener('keydown', bodyHandler);

    const promise = showTextInputDialog({
      title: 'Title',
      initialValue: 'Old task',
      confirmLabel: 'OK',
    });
    const backdrop = document.querySelector('.text-input-dialog-backdrop') as HTMLElement;
    backdrop.dispatchEvent(new KeyboardEvent('keydown', { key: 'a', bubbles: true }));

    expect(bodyHandler).not.toHaveBeenCalled();

    // Clean up
    document.body.removeEventListener('keydown', bodyHandler);
    getCancel().click();
    await promise;
  });
});
