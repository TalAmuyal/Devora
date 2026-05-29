import { describe, it, expect } from 'vitest';
import { createEmptyState } from '../EmptyState';

describe('createEmptyState', () => {
  it('returns a div with class empty-state', () => {
    const el = createEmptyState('No items');
    expect(el.tagName).toBe('DIV');
    expect(el.classList.contains('empty-state')).toBe(true);
  });

  it('sets text content to the message argument', () => {
    const el = createEmptyState('Nothing here');
    expect(el.textContent).toBe('Nothing here');
  });
});
