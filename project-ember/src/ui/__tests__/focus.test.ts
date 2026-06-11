import { describe, it, expect, afterEach } from 'vitest';
import { isEditableElementFocused } from '../focus';

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
