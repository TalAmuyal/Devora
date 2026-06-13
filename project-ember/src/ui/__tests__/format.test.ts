import { describe, it, expect } from 'vitest';
import { pluralize } from '../format';

describe('pluralize', () => {
  it('uses the singular for a count of 1', () => {
    expect(pluralize(1, 'repo')).toBe('1 repo');
  });

  it('uses the default plural (singular + s) for other counts', () => {
    expect(pluralize(0, 'repo')).toBe('0 repos');
    expect(pluralize(2, 'repo')).toBe('2 repos');
  });

  it('uses an explicit plural when provided', () => {
    expect(pluralize(1, 'workspace', 'workspaces')).toBe('1 workspace');
    expect(pluralize(3, 'workspace', 'workspaces')).toBe('3 workspaces');
  });
});
