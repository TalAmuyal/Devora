import { describe, it, expect, afterEach } from 'vitest';
import { createRepoList, RepoListHandle, RepoListMode } from '../RepoList';
import { RepoInfo } from '../../../workspace/types';

const repos: RepoInfo[] = [
  { name: 'repo-a', path: '/src/repo-a', source: 'registered' },
  { name: 'repo-b', path: '/src/repo-b', source: 'auto-discovered' },
];

const manyRepos: RepoInfo[] = [
  { name: 'alpha', path: '/src/alpha', source: 'registered' },
  { name: 'beta', path: '/src/beta', source: 'registered' },
  { name: 'gamma', path: '/src/gamma', source: 'registered' },
];

function mount(mode: RepoListMode, list = repos): RepoListHandle {
  const handle = createRepoList({ repos: list, mode });
  document.body.appendChild(handle.element);
  return handle;
}

function rowInputs(handle: RepoListHandle): HTMLInputElement[] {
  return Array.from(handle.element.querySelectorAll<HTMLInputElement>('.repo-list-item input'));
}

function rowItems(handle: RepoListHandle): HTMLElement[] {
  return Array.from(handle.element.querySelectorAll<HTMLElement>('.repo-list-item'));
}

function visibleNames(handle: RepoListHandle): string[] {
  return rowItems(handle)
    .filter((item) => !item.hidden)
    .map((item) => item.querySelector('.repo-list-item-name')?.textContent ?? '');
}

function filterField(handle: RepoListHandle): HTMLInputElement {
  return handle.element.querySelector('.search-input-field') as HTMLInputElement;
}

function typeFilter(handle: RepoListHandle, value: string): void {
  const field = filterField(handle);
  field.value = value;
  field.dispatchEvent(new Event('input'));
}

function pressKey(handle: RepoListHandle, key: string): void {
  filterField(handle).dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
}

function activeName(handle: RepoListHandle): string | null {
  return (
    handle.element.querySelector('.repo-list-item.active .repo-list-item-name')?.textContent ?? null
  );
}

describe('createRepoList', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('renders one item per repo, showing its name', () => {
    const handle = mount('multi');
    const items = rowItems(handle);
    expect(items.length).toBe(2);
    expect(items[0].textContent).toContain('repo-a');
    expect(items[1].textContent).toContain('repo-b');
  });

  it('multi mode uses checkboxes and returns every checked path', () => {
    const handle = mount('multi');
    const inputs = rowInputs(handle);
    expect(inputs[0].type).toBe('checkbox');
    inputs[0].checked = true;
    inputs[1].checked = true;
    expect(handle.getSelectedPaths()).toEqual(['/src/repo-a', '/src/repo-b']);
  });

  it('multi mode selects nothing by default', () => {
    const handle = mount('multi');
    expect(handle.getSelectedPaths()).toEqual([]);
  });

  it('single mode uses radios sharing one group and returns the selected path', () => {
    const handle = mount('single');
    const inputs = rowInputs(handle);
    expect(inputs[0].type).toBe('radio');
    expect(inputs[0].name).toBe(inputs[1].name);
    expect(inputs[0].name).not.toBe('');

    inputs[1].checked = true;
    expect(handle.getSelectedPaths()).toEqual(['/src/repo-b']);
  });

  it('single mode selects the first repo by default', () => {
    const handle = mount('single');
    expect(handle.getSelectedPaths()).toEqual(['/src/repo-a']);
  });

  it('gives two single-mode lists distinct radio groups', () => {
    const a = mount('single');
    const b = mount('single');
    const aName = rowInputs(a)[0].getAttribute('name');
    const bName = rowInputs(b)[0].getAttribute('name');
    expect(aName).not.toBe(bName);
  });

  it('renders a filter input', () => {
    const handle = mount('multi');
    expect(filterField(handle)).not.toBeNull();
  });

  it('filters by name (case-insensitive substring), hiding non-matching rows', () => {
    const handle = mount('multi', manyRepos);
    typeFilter(handle, 'mm'); // only gamma
    expect(visibleNames(handle)).toEqual(['gamma']);

    typeFilter(handle, 'BET'); // case-insensitive, only beta
    expect(visibleNames(handle)).toEqual(['beta']);

    typeFilter(handle, 'a'); // substring shared by all three
    expect(visibleNames(handle)).toEqual(['alpha', 'beta', 'gamma']);
  });

  it('shows an empty state when nothing matches, and restores on clear', () => {
    const handle = mount('multi', manyRepos);
    const emptyState = handle.element.querySelector('.empty-state') as HTMLElement;

    typeFilter(handle, 'zzz');
    expect(visibleNames(handle)).toEqual([]);
    expect(emptyState.hidden).toBe(false);

    typeFilter(handle, '');
    expect(visibleNames(handle)).toEqual(['alpha', 'beta', 'gamma']);
    expect(emptyState.hidden).toBe(true);
  });

  it('keeps a multi-mode selection checked even when filtered out of view', () => {
    const handle = mount('multi', manyRepos);
    rowInputs(handle)[0].checked = true; // alpha
    typeFilter(handle, 'beta'); // hides alpha
    expect(rowItems(handle)[0].hidden).toBe(true);
    expect(handle.getSelectedPaths()).toEqual(['/src/alpha']);
  });

  it('moves the active highlight with ArrowDown/ArrowUp', () => {
    const handle = mount('multi', manyRepos);
    expect(activeName(handle)).toBe('alpha');
    pressKey(handle, 'ArrowDown');
    expect(activeName(handle)).toBe('beta');
    pressKey(handle, 'ArrowUp');
    expect(activeName(handle)).toBe('alpha');
  });

  it('multi mode toggles the active row checkbox on Enter', () => {
    const handle = mount('multi', manyRepos);
    pressKey(handle, 'ArrowDown'); // active = beta
    pressKey(handle, 'Enter');
    expect(handle.getSelectedPaths()).toEqual(['/src/beta']);
    pressKey(handle, 'Enter'); // toggle back off
    expect(handle.getSelectedPaths()).toEqual([]);
  });

  it('single mode moves the checked radio with the active row', () => {
    const handle = mount('single', manyRepos);
    expect(handle.getSelectedPaths()).toEqual(['/src/alpha']);
    pressKey(handle, 'ArrowDown');
    expect(handle.getSelectedPaths()).toEqual(['/src/beta']);
  });

  it('single mode clears the selection when the filter matches nothing', () => {
    const handle = mount('single', manyRepos);
    typeFilter(handle, 'zzz');
    expect(handle.getSelectedPaths()).toEqual([]);
  });

  it('focus() moves keyboard focus into the filter input', () => {
    const handle = mount('multi');
    handle.focus();
    expect(document.activeElement).toBe(filterField(handle));
  });
});
