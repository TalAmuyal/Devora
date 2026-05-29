import { describe, it, expect, vi } from 'vitest';
import { createSearchInput } from '../SearchInput';

describe('createSearchInput', () => {
  it('returns a handle with element, focus, and getValue', () => {
    const handle = createSearchInput({
      placeholder: 'Search...',
      value: '',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    expect(handle.element).toBeInstanceOf(HTMLElement);
    expect(typeof handle.focus).toBe('function');
    expect(typeof handle.getValue).toBe('function');
  });

  it('DOM contains the icon span and input field', () => {
    const handle = createSearchInput({
      placeholder: 'Filter...',
      value: '',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    const icon = handle.element.querySelector('.search-input-icon');
    const input = handle.element.querySelector('.search-input-field');
    expect(icon).not.toBeNull();
    expect(icon?.textContent).toBe('⌕');
    expect(input).not.toBeNull();
    expect(input?.tagName).toBe('INPUT');
  });

  it('sets placeholder and value from options', () => {
    const handle = createSearchInput({
      placeholder: 'Type here',
      value: 'hello',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    const input = handle.element.querySelector('.search-input-field') as HTMLInputElement;
    expect(input.placeholder).toBe('Type here');
    expect(input.value).toBe('hello');
  });

  it('typing fires onInput with the current value', () => {
    const onInput = vi.fn();
    const handle = createSearchInput({
      placeholder: '',
      value: '',
      icon: '⌕',
      onInput,
      onEscape: () => {},
    });
    const input = handle.element.querySelector('.search-input-field') as HTMLInputElement;
    input.value = 'test';
    input.dispatchEvent(new Event('input'));
    expect(onInput).toHaveBeenCalledWith('test');
  });

  it('pressing Escape in the input calls onEscape', () => {
    const onEscape = vi.fn();
    const handle = createSearchInput({
      placeholder: '',
      value: '',
      icon: '⌕',
      onInput: () => {},
      onEscape,
    });
    const input = handle.element.querySelector('.search-input-field') as HTMLInputElement;
    const event = new KeyboardEvent('keydown', { key: 'Escape' });
    input.dispatchEvent(event);
    expect(onEscape).toHaveBeenCalledOnce();
  });

  it('Escape event is stopped from propagating', () => {
    const handle = createSearchInput({
      placeholder: '',
      value: '',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    const input = handle.element.querySelector('.search-input-field') as HTMLInputElement;
    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
    const parentHandler = vi.fn();
    handle.element.addEventListener('keydown', parentHandler);
    input.dispatchEvent(event);
    expect(parentHandler).not.toHaveBeenCalled();
  });

  it('focus() focuses the input element', () => {
    const handle = createSearchInput({
      placeholder: '',
      value: 'abc',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    document.body.appendChild(handle.element);
    handle.focus();
    const input = handle.element.querySelector('.search-input-field') as HTMLInputElement;
    expect(document.activeElement).toBe(input);
    document.body.removeChild(handle.element);
  });

  it('getValue returns the current input value', () => {
    const handle = createSearchInput({
      placeholder: '',
      value: 'initial',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    expect(handle.getValue()).toBe('initial');
    const input = handle.element.querySelector('.search-input-field') as HTMLInputElement;
    input.value = 'changed';
    expect(handle.getValue()).toBe('changed');
  });

  it('container has search-input class', () => {
    const handle = createSearchInput({
      placeholder: '',
      value: '',
      icon: '⌕',
      onInput: () => {},
      onEscape: () => {},
    });
    expect(handle.element.classList.contains('search-input')).toBe(true);
  });
});
