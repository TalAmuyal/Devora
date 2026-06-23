/**
 * Modal for cloning a git repo into a profile's `repos/` directory, opened from the Command Palette, the Workspace Hub New Task form, or the Settings Hub.
 * One backdrop, two phases: a form (a single URL input + a live "Clones to:" hint) and a progress view (a hosted {@link createTaskCreationProgress}).
 * Pure UI — the caller wires the form submission and the returned progress handle to the backend.
 *
 * DOM: `div.clone-repo-dialog-backdrop > div.clone-repo-dialog`.
 */

import { deriveCloneDirName } from './cloneTarget';
import { createTaskCreationProgress, TaskCreationProgressHandle } from './TaskCreationProgress';

export interface CloneRepoChoice {
  url: string;
}

export interface CloneRepoDialogHandle {
  /** Fired when the user submits the form with a non-empty URL. */
  onSubmit(cb: (choice: CloneRepoChoice) => void): void;
  /** Fired when the user cancels/dismisses during the form phase. */
  onCancel(cb: () => void): void;
  /** Swap the form for a progress view (with `title`) and return a handle to drive it. */
  showProgress(title: string): TaskCreationProgressHandle;
  /** Remove the dialog from the DOM. */
  close(): void;
}

export function showCloneRepoDialog(options: { profilePath: string }): CloneRepoDialogHandle {
  let submitCallback: ((choice: CloneRepoChoice) => void) | null = null;
  let cancelCallback: (() => void) | null = null;
  let phase: 'form' | 'progress' = 'form';
  let closed = false;

  const backdrop = document.createElement('div');
  backdrop.className = 'clone-repo-dialog-backdrop';
  backdrop.addEventListener('click', (e) => {
    if (e.target === backdrop && phase === 'form') cancel();
  });
  // Keep typing inside the dialog from reaching global keyboard handlers.
  backdrop.addEventListener('keydown', (e) => e.stopPropagation());

  const dialog = document.createElement('div');
  dialog.className = 'clone-repo-dialog';
  backdrop.appendChild(dialog);

  const title = document.createElement('div');
  title.className = 'clone-repo-dialog-title';
  title.textContent = 'Clone Repo';
  dialog.appendChild(title);

  const urlLabel = document.createElement('label');
  urlLabel.className = 'clone-repo-dialog-label';
  urlLabel.textContent = 'Repository URL';
  dialog.appendChild(urlLabel);

  const urlInput = document.createElement('input');
  urlInput.type = 'text';
  urlInput.className = 'clone-repo-dialog-input';
  urlInput.placeholder = 'e.g. git@github.com:org/repo.git';
  dialog.appendChild(urlInput);

  const hint = document.createElement('div');
  hint.className = 'clone-repo-dialog-hint';
  dialog.appendChild(hint);

  const reposDir = `${options.profilePath}/repos`;
  const updateHint = (): void => {
    const dirName = deriveCloneDirName(urlInput.value);
    if (dirName) {
      hint.textContent = `Clones to:  ${reposDir}/${dirName}`;
      hint.hidden = false;
    } else {
      hint.textContent = '';
      hint.hidden = true;
    }
  };
  urlInput.addEventListener('input', updateHint);
  updateHint();

  const actions = document.createElement('div');
  actions.className = 'clone-repo-dialog-actions';

  const cancelBtn = document.createElement('button');
  cancelBtn.className = 'clone-repo-dialog-cancel';
  cancelBtn.textContent = 'Cancel';
  cancelBtn.addEventListener('click', () => cancel());
  actions.appendChild(cancelBtn);

  const cloneBtn = document.createElement('button');
  cloneBtn.className = 'clone-repo-dialog-clone';
  cloneBtn.textContent = 'Clone';
  cloneBtn.addEventListener('click', () => submit());
  actions.appendChild(cloneBtn);

  dialog.appendChild(actions);

  function submit(): void {
    if (phase !== 'form') return;
    const url = urlInput.value.trim();
    if (url === '') return;
    submitCallback?.({ url });
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
  urlInput.focus();

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
