/**
 * Modal for creating (or duplicating) a task/workspace: a title input plus a multi-select repo list with an inline "Clone Repo" action.
 * Pure UI — the caller wires task creation, repo cloning, and repo reloading.
 *
 * A `modal` layer (ADR-003): Escape/`q` cancel; Enter creates the task from the title but toggles the active repo when the filter is focused (so the list keeps its keyboard nav).
 * On create the dialog closes first, then hands off — the caller tears the Hub down, so the modal must be gone before that happens.
 * DOM: `div.new-task-dialog-backdrop > div.new-task-dialog`.
 */

import { RepoInfo } from '../../workspace/types';
import { showModalDialog } from './ModalDialog';
import { createRepoList, RepoListHandle } from './RepoList';

export interface NewTaskDialogOptions {
  profilePath: string;
  availableRepos: RepoInfo[];
  initialTitle: string;
  preselectedRepoPaths: string[];
  isDuplicating: boolean;
  /** Create the task from the entered title and selected repo paths. */
  onCreate: (taskName: string, repoPaths: string[]) => void;
  /** Clone a repo into the profile; `onDone` receives the cloned repo so the list refreshes and pre-selects it. */
  onCloneRepo: (profilePath: string, onDone: (repo: { path: string; name: string }) => void) => void;
  /** Re-fetch the profile's registered repos after a clone. */
  reloadRepos: () => Promise<RepoInfo[]>;
}

export interface NewTaskDialogHandle {
  /** Remove the dialog from the DOM. */
  close(): void;
}

export function showNewTaskDialog(options: NewTaskDialogOptions): NewTaskDialogHandle {
  let availableRepos = options.availableRepos;
  let preselectedRepoPaths = options.preselectedRepoPaths;
  let repoListHandle: RepoListHandle | null = null;
  let closed = false;

  const dialog = document.createElement('div');
  dialog.className = 'new-task-dialog';

  const heading = document.createElement('div');
  heading.className = 'new-task-dialog-title';
  heading.textContent = options.isDuplicating ? 'Duplicate Task' : 'New Task';
  dialog.appendChild(heading);

  const nameLabel = document.createElement('label');
  nameLabel.className = 'ws-new-form-label';
  nameLabel.textContent = 'Title';
  dialog.appendChild(nameLabel);

  const nameInput = document.createElement('input');
  nameInput.type = 'text';
  nameInput.className = 'ws-new-form-input';
  nameInput.placeholder = 'e.g. Fix login bug';
  nameInput.value = options.initialTitle;
  dialog.appendChild(nameInput);

  const repoHeader = document.createElement('div');
  repoHeader.className = 'ws-new-form-repo-header';
  const repoLabel = document.createElement('label');
  repoLabel.className = 'ws-new-form-label';
  repoLabel.textContent = 'Repositories';
  repoHeader.appendChild(repoLabel);
  const cloneBtn = document.createElement('button');
  cloneBtn.className = 'ws-new-form-clone';
  cloneBtn.textContent = '+ Clone Repo';
  cloneBtn.addEventListener('click', () => {
    // Capture the in-progress title/selection so the clone-triggered rebuild restores them.
    const typedTitle = nameInput.value;
    const selected = repoListHandle?.getSelectedPaths() ?? [];
    options.onCloneRepo(options.profilePath, (repo) => {
      void refreshAfterClone(typedTitle, selected, repo.path);
    });
  });
  repoHeader.appendChild(cloneBtn);
  dialog.appendChild(repoHeader);

  if (options.isDuplicating) {
    const hint = document.createElement('p');
    hint.className = 'ws-new-form-hint';
    hint.textContent =
      'Pre-selected repos are duplicated at their current commit; added repos use the latest commit.';
    dialog.appendChild(hint);
  }

  const repoSection = document.createElement('div');
  repoSection.className = 'new-task-dialog-repos';
  dialog.appendChild(repoSection);
  renderRepoSection();

  const actions = document.createElement('div');
  actions.className = 'ws-new-form-actions';
  const createBtn = document.createElement('button');
  createBtn.className = 'ws-new-form-create';
  createBtn.textContent = 'Create';
  createBtn.addEventListener('click', () => submit());
  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'ws-new-form-cancel';
  cancelBtn.textContent = 'Cancel';
  cancelBtn.addEventListener('click', () => close());
  actions.appendChild(createBtn);
  actions.appendChild(cancelBtn);
  dialog.appendChild(actions);

  function renderRepoSection(): void {
    repoSection.replaceChildren();
    if (availableRepos.length > 0) {
      repoListHandle = createRepoList({
        repos: availableRepos,
        mode: 'multi',
        preselectedPaths: preselectedRepoPaths,
      });
      repoSection.appendChild(repoListHandle.element);
    } else {
      repoListHandle = null;
      const empty = document.createElement('p');
      empty.className = 'ws-new-form-hint';
      empty.textContent = 'No repos in this profile yet — clone one to add it.';
      repoSection.appendChild(empty);
    }
  }

  async function refreshAfterClone(
    typedTitle: string,
    selected: string[],
    newRepoPath: string,
  ): Promise<void> {
    availableRepos = await options.reloadRepos();
    preselectedRepoPaths = [...new Set([...selected, newRepoPath])];
    nameInput.value = typedTitle;
    renderRepoSection();
  }

  function submit(): void {
    const taskName = nameInput.value.trim();
    if (taskName === '') return;
    const repoPaths = repoListHandle?.getSelectedPaths() ?? [];
    close();
    options.onCreate(taskName, repoPaths);
  }

  function close(): void {
    if (closed) return;
    closed = true;
    modal.close();
  }

  const modal = showModalDialog({
    name: 'new-task-dialog',
    element: dialog,
    onDismiss: () => close(),
    onConfirm: () => {
      // Enter creates from the title, but drives the repo list while its filter is focused.
      if (repoListHandle && repoListHandle.element.contains(document.activeElement)) {
        repoListHandle.toggleActive();
      } else {
        submit();
      }
    },
    resolveFocus: () => nameInput,
  });

  return { close };
}
