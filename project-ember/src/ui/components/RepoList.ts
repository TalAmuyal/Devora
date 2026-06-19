/**
 * A selectable list of repos, shared by the New Task form (multi-select) and the Add Repo dialog
 * (single-select). Kept as one component so future shared affordances (e.g. a filter input) live in
 * a single place.
 *
 * DOM: `div.repo-list > label.repo-list-item > input + span.repo-list-item-name`.
 */

import { RepoInfo } from '../../workspace/types';

export type RepoListMode = 'single' | 'multi';

export interface RepoListOptions {
  repos: RepoInfo[];
  mode: RepoListMode;
}

export interface RepoListHandle {
  element: HTMLElement;
  /** Selected repo paths: at most one in `single` mode, any number in `multi` mode. */
  getSelectedPaths(): string[];
}

/** Distinct radio-group name per `single`-mode instance, so two lists on the page don't share a group. */
let groupCounter = 0;

export function createRepoList(options: RepoListOptions): RepoListHandle {
  const element = document.createElement('div');
  element.className = 'repo-list';

  const inputType = options.mode === 'single' ? 'radio' : 'checkbox';
  const groupName = options.mode === 'single' ? `repo-list-${(groupCounter += 1)}` : undefined;

  for (const repo of options.repos) {
    const item = document.createElement('label');
    item.className = 'repo-list-item';

    const input = document.createElement('input');
    input.type = inputType;
    input.value = repo.path;
    if (groupName) input.name = groupName;

    const name = document.createElement('span');
    name.className = 'repo-list-item-name';
    name.textContent = repo.name;

    item.appendChild(input);
    item.appendChild(name);
    element.appendChild(item);
  }

  return {
    element,
    getSelectedPaths: () =>
      Array.from(element.querySelectorAll<HTMLInputElement>('input:checked')).map(
        (input) => input.value,
      ),
  };
}
