import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { showModalDialog } from '../ModalDialog';
import { getLayerStack } from '../../layers/stack';
import { installModalStack, teardownModalStack, pressKey } from './modalTestHarness';

function content(...children: HTMLElement[]): HTMLElement {
  const el = document.createElement('div');
  el.className = 'dialog-body';
  el.append(...children);
  return el;
}

function button(label: string): HTMLButtonElement {
  const b = document.createElement('button');
  b.textContent = label;
  return b;
}

function wrapper(): HTMLElement {
  return document.querySelector('.test-dialog-backdrop') as HTMLElement;
}

describe('showModalDialog', () => {
  beforeEach(() => installModalStack());
  afterEach(() => teardownModalStack());

  it('mounts the element inside a `layer-modal ${name}-backdrop` wrapper on the stack', () => {
    const el = content();
    showModalDialog({ name: 'test-dialog', element: el, onDismiss: vi.fn() });

    const w = wrapper();
    expect(w).not.toBeNull();
    expect(w.classList.contains('layer-modal')).toBe(true);
    expect(w.contains(el)).toBe(true);
    expect(getLayerStack().topOf('modal')).not.toBeNull();
  });

  it('Enter confirms', () => {
    const onConfirm = vi.fn();
    showModalDialog({ name: 'test-dialog', element: content(), onDismiss: vi.fn(), onConfirm });

    const e = pressKey({ key: 'Enter', code: 'Enter' });

    expect(onConfirm).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('Escape and q both cancel when focus is not on an input', () => {
    const onDismiss = vi.fn();
    showModalDialog({ name: 'test-dialog', element: content(button('OK')), onDismiss });

    pressKey({ key: 'Escape', code: 'Escape' });
    expect(onDismiss).toHaveBeenCalledTimes(1);

    pressKey({ key: 'q', code: 'KeyQ' });
    expect(onDismiss).toHaveBeenCalledTimes(2);
  });

  it('q types into a focused input instead of dismissing', () => {
    const onDismiss = vi.fn();
    const input = document.createElement('input');
    showModalDialog({
      name: 'test-dialog',
      element: content(input),
      onDismiss,
      resolveFocus: () => input,
    });
    expect(document.activeElement).toBe(input);

    const e = pressKey({ key: 'q', code: 'KeyQ' });

    expect(onDismiss).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false); // unconsumed so the character is typed
  });

  it('Escape still cancels even with an input focused (modals own Escape)', () => {
    const onDismiss = vi.fn();
    const input = document.createElement('input');
    showModalDialog({
      name: 'test-dialog',
      element: content(input),
      onDismiss,
      resolveFocus: () => input,
    });

    const e = pressKey({ key: 'Escape', code: 'Escape' });

    expect(onDismiss).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('a backdrop click cancels; a click inside the dialog does not', () => {
    const onDismiss = vi.fn();
    const el = content(button('OK'));
    showModalDialog({ name: 'test-dialog', element: el, onDismiss });

    el.click();
    expect(onDismiss).not.toHaveBeenCalled();

    wrapper().click();
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('onBackdropClick overrides the backdrop-click target when provided', () => {
    const onDismiss = vi.fn();
    const onBackdropClick = vi.fn();
    showModalDialog({ name: 'test-dialog', element: content(), onDismiss, onBackdropClick });

    wrapper().click();

    expect(onBackdropClick).toHaveBeenCalledOnce();
    expect(onDismiss).not.toHaveBeenCalled();
  });

  it('Tab cycles focus within the modal, wrapping at the ends', () => {
    const first = button('First');
    const second = button('Second');
    showModalDialog({
      name: 'test-dialog',
      element: content(first, second),
      onDismiss: vi.fn(),
      resolveFocus: () => first,
    });
    expect(document.activeElement).toBe(first);

    pressKey({ key: 'Tab', code: 'Tab' });
    expect(document.activeElement).toBe(second);

    pressKey({ key: 'Tab', code: 'Tab' });
    expect(document.activeElement).toBe(first);
  });

  it('close() restores focus to the revealed page beneath', () => {
    const revealedFocus = { focus: vi.fn() };
    getLayerStack().push({
      name: 'page-beneath',
      kind: 'page',
      element: content(),
      resolveFocus: () => revealedFocus,
    });
    const handle = showModalDialog({ name: 'test-dialog', element: content(), onDismiss: vi.fn() });

    const before = revealedFocus.focus.mock.calls.length;
    handle.close();

    expect(revealedFocus.focus.mock.calls.length).toBeGreaterThan(before);
    expect(getLayerStack().topOf('modal')).toBeNull();
  });

  it('runs onCleanup exactly once when the modal is removed', () => {
    const onCleanup = vi.fn();
    const handle = showModalDialog({
      name: 'test-dialog',
      element: content(),
      onDismiss: vi.fn(),
      onCleanup,
    });

    handle.close();
    handle.close(); // idempotent

    expect(onCleanup).toHaveBeenCalledOnce();
  });

  it('a two-phase dialog forwards Escape to its .task-creation-action (progress phase)', () => {
    const action = button('Cancel');
    action.className = 'task-creation-action';
    const onAction = vi.fn();
    action.addEventListener('click', onAction);
    // onDismiss mirrors the progress-phase branch of the two-phase dialogs.
    showModalDialog({
      name: 'test-dialog',
      element: content(action),
      onDismiss: () => action.click(),
    });

    pressKey({ key: 'Escape', code: 'Escape' });

    expect(onAction).toHaveBeenCalledOnce();
  });
});
