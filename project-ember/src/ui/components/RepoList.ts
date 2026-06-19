/**
 * A selectable, filterable list of repos, shared by the New Task form (multi-select) and the Add Repo dialog (single-select).
 * A name filter with keyboard navigation lives here so both consumers share it.
 *
 * DOM: `div.repo-list > div.search-input + div.repo-list-items`, where `.repo-list-items` holds `label.repo-list-item > input + span.repo-list-item-name` rows plus a hidden `div.empty-state`.
 * Filtered-out rows are hidden (not removed), so checked state survives filtering.
 *
 * Keyboard (while the filter input is focused): typing filters by name (case-insensitive substring), Up/Down move the active row.
 * In `single` mode the active row is the checked radio; in `multi` mode Enter toggles the active row's checkbox.
 */

import { RepoInfo } from '../../workspace/types';
import { createSearchInput } from './SearchInput';
import { createEmptyState } from './EmptyState';
import { createListCursor } from './listCursor';

export type RepoListMode = 'single' | 'multi';

export interface RepoListOptions {
  repos: RepoInfo[];
  mode: RepoListMode;
}

export interface RepoListHandle {
  element: HTMLElement;
  /** Selected repo paths: at most one in `single` mode, any number in `multi` mode. */
  getSelectedPaths(): string[];
  /** Move keyboard focus into the filter input. */
  focus(): void;
}

/** Distinct radio-group name per `single`-mode instance, so two lists on the page don't share a group. */
let groupCounter = 0;

interface Row {
  repo: RepoInfo;
  item: HTMLLabelElement;
  input: HTMLInputElement;
}

export function createRepoList(options: RepoListOptions): RepoListHandle {
  const element = document.createElement('div');
  element.className = 'repo-list';

  const inputType = options.mode === 'single' ? 'radio' : 'checkbox';
  const groupName = options.mode === 'single' ? `repo-list-${(groupCounter += 1)}` : undefined;

  const itemsContainer = document.createElement('div');
  itemsContainer.className = 'repo-list-items';

  const rows: Row[] = options.repos.map((repo) => {
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
    itemsContainer.appendChild(item);
    return { repo, item, input };
  });

  const emptyState = createEmptyState('No matching repos');
  itemsContainer.appendChild(emptyState);

  const visibleItems = (): HTMLElement[] => rows.filter((r) => !r.item.hidden).map((r) => r.item);

  const cursor = createListCursor({
    items: visibleItems,
    activeClass: 'active',
    // Single-select: the active row is the selection, so keep exactly its radio checked.
    onChange:
      options.mode === 'single'
        ? (active) => {
            if (active) {
              // Checking a radio unchecks its group siblings, including any now-hidden one.
              active.querySelector<HTMLInputElement>('input')!.checked = true;
            } else {
              for (const row of rows) row.input.checked = false;
            }
          }
        : undefined,
  });

  function applyFilter(value: string): void {
    const needle = value.trim().toLowerCase();
    for (const row of rows) {
      row.item.hidden = needle !== '' && !row.repo.name.toLowerCase().includes(needle);
    }
    emptyState.hidden = rows.some((r) => !r.item.hidden);
    cursor.set(0); // reset the active row (and single-mode selection) to the first visible match
  }

  const search = createSearchInput({
    placeholder: 'Filter repos…',
    value: '',
    icon: '⌕',
    onInput: applyFilter,
    onEscape: () => {},
    onArrowDown: () => cursor.move(1),
    onArrowUp: () => cursor.move(-1),
    onEnter: () => {
      if (options.mode === 'multi') {
        const input = cursor.active()?.querySelector<HTMLInputElement>('input');
        if (input) input.checked = !input.checked;
      }
    },
  });

  // Clicking a row moves the keyboard cursor onto it (its native input toggle still fires).
  for (const row of rows) {
    row.item.addEventListener('click', () => {
      const index = visibleItems().indexOf(row.item);
      if (index >= 0) cursor.set(index);
    });
  }

  element.appendChild(search.element);
  element.appendChild(itemsContainer);

  emptyState.hidden = rows.length > 0;
  cursor.set(0); // initial highlight; in single mode this selects the first repo

  return {
    element,
    getSelectedPaths: () =>
      Array.from(itemsContainer.querySelectorAll<HTMLInputElement>('input:checked')).map(
        (input) => input.value,
      ),
    focus: () => search.focus(),
  };
}
