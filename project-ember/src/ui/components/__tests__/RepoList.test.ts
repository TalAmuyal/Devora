import { describe, it, expect } from 'vitest';
import { createRepoList } from '../RepoList';
import { RepoInfo } from '../../../workspace/types';

const repos: RepoInfo[] = [
  { name: 'repo-a', path: '/src/repo-a', source: 'registered' },
  { name: 'repo-b', path: '/src/repo-b', source: 'auto-discovered' },
];

describe('createRepoList', () => {
  it('renders one item per repo, showing its name', () => {
    const { element } = createRepoList({ repos, mode: 'multi' });
    const items = element.querySelectorAll('.repo-list-item');
    expect(items.length).toBe(2);
    expect(items[0].textContent).toContain('repo-a');
    expect(items[1].textContent).toContain('repo-b');
  });

  it('multi mode uses checkboxes and returns every checked path', () => {
    const handle = createRepoList({ repos, mode: 'multi' });
    const inputs = handle.element.querySelectorAll<HTMLInputElement>('input');
    expect(inputs[0].type).toBe('checkbox');
    inputs[0].checked = true;
    inputs[1].checked = true;
    expect(handle.getSelectedPaths()).toEqual(['/src/repo-a', '/src/repo-b']);
  });

  it('single mode uses radios sharing one group and returns the selected path', () => {
    const handle = createRepoList({ repos, mode: 'single' });
    const inputs = handle.element.querySelectorAll<HTMLInputElement>('input');
    expect(inputs[0].type).toBe('radio');
    expect(inputs[0].name).toBe(inputs[1].name);
    expect(inputs[0].name).not.toBe('');

    inputs[1].checked = true;
    expect(handle.getSelectedPaths()).toEqual(['/src/repo-b']);
  });

  it('returns no paths when nothing is selected', () => {
    const handle = createRepoList({ repos, mode: 'single' });
    expect(handle.getSelectedPaths()).toEqual([]);
  });

  it('gives two single-mode lists distinct radio groups', () => {
    const a = createRepoList({ repos, mode: 'single' });
    const b = createRepoList({ repos, mode: 'single' });
    const aName = a.element.querySelector('input')!.getAttribute('name');
    const bName = b.element.querySelector('input')!.getAttribute('name');
    expect(aName).not.toBe(bName);
  });
});
