import { describe, it, expect, afterEach } from 'vitest';
import { isEditableElementFocused, blurOnEscape } from '../focus';

describe('isEditableElementFocused', () => {
  afterEach(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.innerHTML = '';
  });

  function mount<T extends HTMLElement>(el: T): T {
    document.body.appendChild(el);
    return el;
  }

  it('returns false when nothing is focused', () => {
    expect(isEditableElementFocused()).toBe(false);
  });

  it('returns true when an input is focused', () => {
    mount(document.createElement('input')).focus();
    expect(isEditableElementFocused()).toBe(true);
  });

  it('returns true when a textarea is focused', () => {
    mount(document.createElement('textarea')).focus();
    expect(isEditableElementFocused()).toBe(true);
  });

  it('returns true when a select is focused', () => {
    mount(document.createElement('select')).focus();
    expect(isEditableElementFocused()).toBe(true);
  });

  it('returns true when a contentEditable element is focused', () => {
    const editable = mount(document.createElement('div'));
    editable.contentEditable = 'true';
    editable.focus();
    expect(isEditableElementFocused()).toBe(true);
  });

  it('returns false when a non-editable element is focused', () => {
    const div = mount(document.createElement('div'));
    div.tabIndex = -1;
    div.focus();
    expect(isEditableElementFocused()).toBe(false);
  });
});

describe('blurOnEscape', () => {
  afterEach(() => {
    (document.activeElement as HTMLElement | null)?.blur?.();
    document.body.innerHTML = '';
  });

  function mountInput(): HTMLInputElement {
    const input = document.createElement('input');
    document.body.appendChild(input);
    blurOnEscape(input);
    input.focus();
    return input;
  }

  function press(input: HTMLInputElement, key: string): KeyboardEvent {
    const e = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true });
    input.dispatchEvent(e);
    return e;
  }

  it('blurs the input and prevents default on Escape', () => {
    const input = mountInput();
    expect(document.activeElement).toBe(input);
    const e = press(input, 'Escape');
    expect(document.activeElement).not.toBe(input);
    expect(e.defaultPrevented).toBe(true);
  });

  it('leaves focus and default alone for other keys', () => {
    const input = mountInput();
    const e = press(input, 'a');
    expect(document.activeElement).toBe(input);
    expect(e.defaultPrevented).toBe(false);
  });
});
