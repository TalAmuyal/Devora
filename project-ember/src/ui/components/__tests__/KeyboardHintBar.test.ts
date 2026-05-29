import { describe, it, expect } from 'vitest';
import { createKeyboardHintBar } from '../KeyboardHintBar';

describe('createKeyboardHintBar', () => {
  it('renders correct number of hint items', () => {
    const el = createKeyboardHintBar({
      hints: [
        { keys: 'j/k', description: 'navigate' },
        { keys: 'Enter', description: 'open' },
        { keys: 'q', description: 'close' },
      ],
    });
    const items = el.querySelectorAll('.keyboard-hint-item');
    expect(items.length).toBe(3);
  });

  it('each item contains a kbd element with the key text', () => {
    const el = createKeyboardHintBar({
      hints: [
        { keys: 'Enter', description: 'open' },
      ],
    });
    const kbd = el.querySelector('kbd');
    expect(kbd).not.toBeNull();
    expect(kbd?.textContent).toBe('Enter');
  });

  it('each item contains the description text', () => {
    const el = createKeyboardHintBar({
      hints: [
        { keys: 'f', description: 'filter' },
      ],
    });
    const item = el.querySelector('.keyboard-hint-item');
    expect(item?.textContent).toContain('filter');
  });

  it('appends trailing element when provided', () => {
    const trailing = document.createElement('button');
    trailing.textContent = 'Extra';
    trailing.className = 'test-trailing';
    const el = createKeyboardHintBar({
      hints: [{ keys: 'a', description: 'action' }],
      trailing,
    });
    const trailingEl = el.querySelector('.test-trailing');
    expect(trailingEl).not.toBeNull();
    expect(trailingEl?.textContent).toBe('Extra');
  });

  it('no trailing element when omitted', () => {
    const el = createKeyboardHintBar({
      hints: [{ keys: 'a', description: 'action' }],
    });
    expect(el.children.length).toBe(1);
  });

  it('has keyboard-hint-bar class on the container', () => {
    const el = createKeyboardHintBar({
      hints: [{ keys: 'a', description: 'action' }],
    });
    expect(el.classList.contains('keyboard-hint-bar')).toBe(true);
  });
});
