import { describe, it, expect } from 'vitest';
import { createBadge, BadgeVariant } from '../Badge';

describe('createBadge', () => {
  it('returns a span with class badge', () => {
    const el = createBadge('clean', 'clean');
    expect(el.tagName).toBe('SPAN');
    expect(el.classList.contains('badge')).toBe(true);
  });

  it('sets text content to the text argument', () => {
    const el = createBadge('✓ clean', 'clean');
    expect(el.textContent).toBe('✓ clean');
  });

  const variants: BadgeVariant[] = ['clean', 'modified', 'untracked', 'pending', 'error', 'inactive'];

  variants.forEach((variant) => {
    it(`applies badge-${variant} class for ${variant} variant`, () => {
      const el = createBadge('test', variant);
      expect(el.classList.contains(`badge-${variant}`)).toBe(true);
    });

    it(`has exactly two classes for ${variant} variant`, () => {
      const el = createBadge('test', variant);
      expect(el.classList.length).toBe(2);
    });
  });
});
