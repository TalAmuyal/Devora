import { describe, it, expect } from 'vitest';
import { createTableShell } from '../TableShell';

describe('createTableShell', () => {
  it('renders a header cell per column label, in order', () => {
    const { table } = createTableShell(['Repo', 'Source', 'Path']);
    const headers = Array.from(table.querySelectorAll('thead th')).map((th) => th.textContent);
    expect(headers).toEqual(['Repo', 'Source', 'Path']);
  });

  it('returns an empty tbody for the caller to fill', () => {
    const { table, tbody } = createTableShell(['A']);
    expect(tbody.tagName).toBe('TBODY');
    expect(tbody.children.length).toBe(0);
    expect(table.querySelector('tbody')).toBe(tbody);
  });

  it('applies the className when provided and none otherwise', () => {
    expect(createTableShell(['A'], 'pm-repo-table').table.className).toBe('pm-repo-table');
    expect(createTableShell(['A']).table.className).toBe('');
  });
});
