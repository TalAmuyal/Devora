import { describe, it, expect } from 'vitest';
import { createStatusDot, StatusDotVariant } from '../StatusDot';

describe('createStatusDot', () => {
  it('returns a div with class status-dot', () => {
    const el = createStatusDot('clean');
    expect(el.tagName).toBe('DIV');
    expect(el.classList.contains('status-dot')).toBe(true);
  });

  const variants: StatusDotVariant[] = ['clean', 'modified', 'pending', 'error'];

  variants.forEach((variant) => {
    it(`applies the ${variant} variant class`, () => {
      const el = createStatusDot(variant);
      expect(el.classList.contains(variant)).toBe(true);
    });

    it(`has exactly two classes for ${variant} variant`, () => {
      const el = createStatusDot(variant);
      expect(el.classList.length).toBe(2);
    });
  });
});
