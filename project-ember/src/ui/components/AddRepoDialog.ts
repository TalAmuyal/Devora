/**
 * Modal for adding a repo (git worktree) to the current workspace, opened from the Command Palette.
 * One backdrop, two phases: a form (single-select repo + optional name postfix) and a progress view (a hosted {@link createTaskCreationProgress}).
 * Pure UI — the caller wires the form submission and the returned progress handle to the backend.
 *
 * DOM: `div.add-repo-dialog-backdrop > div.add-repo-dialog`.
 */

import { RepoInfo } from '../../workspace/types';
import { createRepoList } from './RepoList';
import { createTaskCreationProgress, TaskCreationProgressHandle } from './TaskCreationProgress';

export interface AddRepoChoice {
  sourceRepoPath: string;
  worktreeDirName: string;
}

export interface AddRepoDialogHandle {
  /** Fired when the user submits the form with a repo selected. */
  onSubmit(cb: (choice: AddRepoChoice) => void): void;
  /** Fired when the user cancels/dismisses during the form phase. */
  onCancel(cb: () => void): void;
  /** Swap the form for a progress view (with `title`) and return a handle to drive it. */
  showProgress(title: string): TaskCreationProgressHandle;
  /** Remove the dialog from the DOM. */
  close(): void;
}

export function showAddRepoDialog(options: { repos: RepoInfo[] }): AddRepoDialogHandle {
  let submitCallback: ((choice: AddRepoChoice) => void) | null = null;
  let cancelCallback: (() => void) | null = null;
  let phase: 'form' | 'progress' = 'form';
  let closed = false;

  const backdrop = document.createElement('div');
  backdrop.className = 'add-repo-dialog-backdrop';
  backdrop.addEventListener('click', (e) => {
    if (e.target === backdrop && phase === 'form') cancel();
  });
  // Keep typing inside the dialog from reaching global keyboard handlers.
  backdrop.addEventListener('keydown', (e) => e.stopPropagation());

  const dialog = document.createElement('div');
  dialog.className = 'add-repo-dialog';
  backdrop.appendChild(dialog);

  const title = document.createElement('div');
  title.className = 'add-repo-dialog-title';
  title.textContent = 'Add Repo';
  dialog.appendChild(title);

  const repoLabel = document.createElement('label');
  repoLabel.className = 'add-repo-dialog-label';
  repoLabel.textContent = 'Repository';
  dialog.appendChild(repoLabel);

  // Single-select RepoList defaults to selecting the first repo, so "Add Repo" is always actionable.
  const repoList = createRepoList({ repos: options.repos, mode: 'single', preselectedPaths: [] });
  dialog.appendChild(repoList.element);

  const postfixLabel = document.createElement('label');
  postfixLabel.className = 'add-repo-dialog-label';
  postfixLabel.textContent = 'Name postfix (optional)';
  dialog.appendChild(postfixLabel);

  const postfixInput = document.createElement('input');
  postfixInput.type = 'text';
  postfixInput.className = 'add-repo-dialog-input';
  postfixInput.placeholder = 'e.g. ref';
  dialog.appendChild(postfixInput);

  const actions = document.createElement('div');
  actions.className = 'add-repo-dialog-actions';

  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'add-repo-dialog-cancel';
  cancelBtn.textContent = 'Cancel';
  cancelBtn.addEventListener('click', () => cancel());
  actions.appendChild(cancelBtn);

  const addBtn = document.createElement('button');
  addBtn.className = 'add-repo-dialog-add';
  addBtn.textContent = 'Add Repo';
  addBtn.addEventListener('click', () => submit());
  actions.appendChild(addBtn);

  dialog.appendChild(actions);

  function submit(): void {
    if (phase !== 'form') return;
    const selectedPath = repoList.getSelectedPaths()[0];
    if (!selectedPath) return;
    const repo = options.repos.find((r) => r.path === selectedPath);
    if (!repo) return;
    const postfix = postfixInput.value.trim();
    const worktreeDirName = postfix ? `${repo.name}-${postfix}` : repo.name;
    submitCallback?.({ sourceRepoPath: repo.path, worktreeDirName });
  }

  function cancel(): void {
    cancelCallback?.();
    close();
  }

  function close(): void {
    if (closed) return;
    closed = true;
    document.removeEventListener('keydown', onKeydown, true);
    backdrop.remove();
  }

  // Capture phase so Enter/Escape are handled before any global shortcut.
  const onKeydown = (e: KeyboardEvent): void => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (phase === 'form') {
        cancel();
      } else {
        // Route to the progress footer action (cancel while running, close after a failure).
        dialog.querySelector<HTMLButtonElement>('.task-creation-action')?.click();
      }
    } else if (e.key === 'Enter' && phase === 'form') {
      e.preventDefault();
      e.stopPropagation();
      submit();
    }
  };
  document.addEventListener('keydown', onKeydown, true);

  document.body.appendChild(backdrop);
  repoList.focus();

  return {
    onSubmit: (cb) => {
      submitCallback = cb;
    },
    onCancel: (cb) => {
      cancelCallback = cb;
    },
    showProgress: (titleText) => {
      phase = 'progress';
      dialog.replaceChildren();
      const progress = createTaskCreationProgress(titleText);
      dialog.appendChild(progress.element);
      return progress;
    },
    close,
  };
}
