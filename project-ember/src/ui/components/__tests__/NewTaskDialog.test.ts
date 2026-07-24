import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { showNewTaskDialog, NewTaskDialogOptions } from '../NewTaskDialog';
import { installModalStack, teardownModalStack, pressKey } from './modalTestHarness';
import type { RepoInfo } from '../../../workspace/types';

beforeEach(() => installModalStack());
afterEach(() => teardownModalStack());

const REPOS: RepoInfo[] = [
  { name: 'alpha', path: '/repos/alpha', source: 'registered' },
  { name: 'beta', path: '/repos/beta', source: 'registered' },
];

function open(overrides: Partial<NewTaskDialogOptions> = {}): {
  handle: ReturnType<typeof showNewTaskDialog>;
  onCreate: ReturnType<typeof vi.fn>;
  onCloneRepo: ReturnType<typeof vi.fn>;
} {
  const onCreate = vi.fn();
  const onCloneRepo = vi.fn();
  const handle = showNewTaskDialog({
    profilePath: '/profiles/work',
    availableRepos: REPOS,
    initialTitle: '',
    preselectedRepoPaths: [],
    isDuplicating: false,
    onCreate,
    onCloneRepo,
    reloadRepos: async () => REPOS,
    ...overrides,
  });
  return { handle, onCreate, onCloneRepo };
}

function backdrop(): HTMLElement | null {
  return document.querySelector('.new-task-dialog-backdrop');
}

function titleInput(): HTMLInputElement {
  return document.querySelector('.new-task-dialog .ws-new-form-input') as HTMLInputElement;
}

function checkedRepoPaths(): string[] {
  return Array.from(
    document.querySelectorAll<HTMLInputElement>('.new-task-dialog .repo-list input:checked'),
  ).map((i) => i.value);
}

describe('showNewTaskDialog', () => {
  it('renders as a modal focused on the title input, pre-filled with the initial title', () => {
    const { handle } = open({ initialTitle: 'Fix login' });
    expect(backdrop()).not.toBeNull();
    expect(document.activeElement).toBe(titleInput());
    expect(titleInput().value).toBe('Fix login');
    handle.close();
  });

  it('Create submits the trimmed title with the selected repo paths, then closes', () => {
    const { onCreate } = open({ preselectedRepoPaths: ['/repos/beta'] });
    titleInput().value = '  My task  ';
    document.querySelector<HTMLButtonElement>('.new-task-dialog .ws-new-form-create')!.click();
    expect(onCreate).toHaveBeenCalledWith('My task', ['/repos/beta']);
    expect(backdrop()).toBeNull();
  });

  it('Create is inert when the title is empty', () => {
    const { onCreate, handle } = open();
    document.querySelector<HTMLButtonElement>('.new-task-dialog .ws-new-form-create')!.click();
    expect(onCreate).not.toHaveBeenCalled();
    expect(backdrop()).not.toBeNull();
    handle.close();
  });

  it('Cancel closes without creating', () => {
    const { onCreate } = open();
    document.querySelector<HTMLButtonElement>('.new-task-dialog .ws-new-form-cancel')!.click();
    expect(onCreate).not.toHaveBeenCalled();
    expect(backdrop()).toBeNull();
  });

  it('Escape closes without creating', () => {
    const { onCreate } = open();
    pressKey({ key: 'Escape', code: 'Escape' });
    expect(onCreate).not.toHaveBeenCalled();
    expect(backdrop()).toBeNull();
  });

  it('duplication prefill pre-checks repos and shows the duplication hint', () => {
    const { handle } = open({ isDuplicating: true, preselectedRepoPaths: ['/repos/alpha'] });
    expect(checkedRepoPaths()).toEqual(['/repos/alpha']);
    expect(document.querySelector('.new-task-dialog .ws-new-form-hint')).not.toBeNull();
    handle.close();
  });

  it('cloning a repo refreshes the list, preserving the typed title and prior selection and pre-selecting the new repo', async () => {
    const withGamma: RepoInfo[] = [...REPOS, { name: 'gamma', path: '/repos/gamma', source: 'registered' }];
    let cloneDone: ((repo: { path: string; name: string }) => void) | null = null;
    const onCloneRepo = vi.fn((_profile: string, onDone: (repo: { path: string; name: string }) => void) => {
      cloneDone = onDone;
    });
    const handle = showNewTaskDialog({
      profilePath: '/profiles/work',
      availableRepos: REPOS,
      initialTitle: '',
      preselectedRepoPaths: [],
      isDuplicating: false,
      onCreate: vi.fn(),
      onCloneRepo,
      reloadRepos: async () => withGamma,
    });

    titleInput().value = 'Typed while cloning';
    // Check an existing repo before cloning — the rebuild must keep it selected.
    const alpha = document.querySelector<HTMLInputElement>(
      '.new-task-dialog .repo-list input[value="/repos/alpha"]',
    )!;
    alpha.checked = true;

    document.querySelector<HTMLButtonElement>('.new-task-dialog .ws-new-form-clone')!.click();
    expect(onCloneRepo).toHaveBeenCalledOnce();

    cloneDone!({ path: '/repos/gamma', name: 'gamma' });
    await Promise.resolve();
    await Promise.resolve();

    expect(titleInput().value).toBe('Typed while cloning');
    expect(checkedRepoPaths().sort()).toEqual(['/repos/alpha', '/repos/gamma']);
    handle.close();
  });

  it('Enter creates when the title is focused, but toggles the active repo when the filter is focused', () => {
    const { onCreate, handle } = open();

    const search = document.querySelector<HTMLInputElement>('.new-task-dialog .repo-list .search-input input')!;
    search.focus();
    pressKey({ key: 'Enter', code: 'Enter' });
    expect(onCreate).not.toHaveBeenCalled();
    expect(checkedRepoPaths()).toEqual(['/repos/alpha']);

    titleInput().value = 'Go';
    titleInput().focus();
    pressKey({ key: 'Enter', code: 'Enter' });
    expect(onCreate).toHaveBeenCalledWith('Go', ['/repos/alpha']);
    void handle;
  });
});
