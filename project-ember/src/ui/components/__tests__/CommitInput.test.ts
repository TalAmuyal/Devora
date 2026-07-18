import { describe, it, expect, vi, afterEach } from 'vitest';
import { createCommitInput } from '../CommitInput';

afterEach(() => {
  document.body.innerHTML = '';
});

function mount(options: Parameters<typeof createCommitInput>[0]) {
  const handle = createCommitInput(options);
  document.body.appendChild(handle.root);
  const save = handle.root.querySelector<HTMLButtonElement>('.config-save')!;
  return { ...handle, save };
}

function setValue(input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('createCommitInput', () => {
  it('renders the input with the given value and placeholder', () => {
    const { input } = mount({ value: 'nvim', placeholder: 'app', savedValue: '', onCommit: vi.fn() });
    expect(input.value).toBe('nvim');
    expect(input.placeholder).toBe('app');
  });

  it('hides the Save button while the value equals the saved value', () => {
    const { save } = mount({ value: 'nvim', savedValue: 'nvim', onCommit: vi.fn() });
    expect(save.hidden).toBe(true);
  });

  it('hides the Save button while the value is empty', () => {
    const { save } = mount({ value: '', savedValue: 'nvim', onCommit: vi.fn() });
    expect(save.hidden).toBe(true);
  });

  it('shows the Save button once the value differs from the saved value', () => {
    const { input, save } = mount({ value: '', savedValue: '', onCommit: vi.fn() });
    expect(save.hidden).toBe(true);
    setValue(input, 'nvim');
    expect(save.hidden).toBe(false);
  });

  it('treats a whitespace-only value as empty', () => {
    const { input, save } = mount({ value: '', savedValue: 'nvim', onCommit: vi.fn() });
    setValue(input, '   ');
    expect(save.hidden).toBe(true);
  });

  it('hides the Save button again when the value is edited back to the saved value', () => {
    const { input, save } = mount({ value: 'nvim', savedValue: 'nvim', onCommit: vi.fn() });
    setValue(input, 'nvi');
    expect(save.hidden).toBe(false);
    setValue(input, 'nvim');
    expect(save.hidden).toBe(true);
  });

  it('commits the current value when the Save button is clicked', () => {
    const onCommit = vi.fn();
    const { input, save } = mount({ value: '', savedValue: '', onCommit });
    setValue(input, 'nvim');
    save.click();
    expect(onCommit).toHaveBeenCalledWith('nvim');
  });

  it('commits the current value and swallows the event when Enter is pressed', () => {
    const onCommit = vi.fn();
    const { input } = mount({ value: 'nvim', savedValue: '', onCommit });
    const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true });
    input.dispatchEvent(event);
    expect(onCommit).toHaveBeenCalledWith('nvim');
    expect(event.defaultPrevented).toBe(true);
  });

  it('blurs the input on Escape', () => {
    const { input } = mount({ value: 'nvim', savedValue: 'nvim', onCommit: vi.fn() });
    input.focus();
    expect(document.activeElement).toBe(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(document.activeElement).not.toBe(input);
  });
});
