import { describe, it, expect, vi, afterEach } from 'vitest';
import { showAddRepoDialog } from '../AddRepoDialog';
import { RepoInfo } from '../../../workspace/types';

const repos: RepoInfo[] = [
  { name: 'repo-a', path: '/src/repo-a', source: 'registered' },
  { name: 'repo-b', path: '/src/repo-b', source: 'registered' },
];

afterEach(() => {
  document.querySelectorAll('.add-repo-dialog-backdrop').forEach((el) => el.remove());
});

function backdrop(): HTMLElement | null {
  return document.querySelector('.add-repo-dialog-backdrop');
}

function radios(): NodeListOf<HTMLInputElement> {
  return document.querySelectorAll<HTMLInputElement>('.add-repo-dialog input[type="radio"]');
}

describe('showAddRepoDialog', () => {
  it('lists the repos and pre-selects the first', () => {
    const handle = showAddRepoDialog({ repos });
    expect(document.querySelectorAll('.repo-list-item').length).toBe(2);
    expect(radios()[0].checked).toBe(true);
    handle.close();
  });

  it('submits the selected repo with no postfix', () => {
    const handle = showAddRepoDialog({ repos });
    const onSubmit = vi.fn();
    handle.onSubmit(onSubmit);
    document.querySelector<HTMLButtonElement>('.add-repo-dialog-add')!.click();
    expect(onSubmit).toHaveBeenCalledWith({
      sourceRepoPath: '/src/repo-a',
      worktreeDirName: 'repo-a',
    });
    handle.close();
  });

  it('appends a trimmed postfix to the worktree dir name', () => {
    const handle = showAddRepoDialog({ repos });
    const onSubmit = vi.fn();
    handle.onSubmit(onSubmit);
    (document.querySelector('.add-repo-dialog-input') as HTMLInputElement).value = ' ref ';
    document.querySelector<HTMLButtonElement>('.add-repo-dialog-add')!.click();
    expect(onSubmit).toHaveBeenCalledWith({
      sourceRepoPath: '/src/repo-a',
      worktreeDirName: 'repo-a-ref',
    });
    handle.close();
  });

  it('submits the explicitly chosen repo', () => {
    const handle = showAddRepoDialog({ repos });
    const onSubmit = vi.fn();
    handle.onSubmit(onSubmit);
    const inputs = radios();
    inputs[0].checked = false;
    inputs[1].checked = true;
    document.querySelector<HTMLButtonElement>('.add-repo-dialog-add')!.click();
    expect(onSubmit).toHaveBeenCalledWith({
      sourceRepoPath: '/src/repo-b',
      worktreeDirName: 'repo-b',
    });
    handle.close();
  });

  it('cancel fires onCancel and removes the dialog', () => {
    const handle = showAddRepoDialog({ repos });
    const onCancel = vi.fn();
    handle.onCancel(onCancel);
    document.querySelector<HTMLButtonElement>('.add-repo-dialog-cancel')!.click();
    expect(onCancel).toHaveBeenCalledOnce();
    expect(backdrop()).toBeNull();
  });

  it('Escape during the form fires onCancel and closes', () => {
    const handle = showAddRepoDialog({ repos });
    const onCancel = vi.fn();
    handle.onCancel(onCancel);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    expect(onCancel).toHaveBeenCalledOnce();
    expect(backdrop()).toBeNull();
  });

  it('showProgress swaps to a progress view and hides the form', () => {
    const handle = showAddRepoDialog({ repos });
    const progress = handle.showProgress('Adding: repo-a');
    expect(document.querySelector('.add-repo-dialog-add')).toBeNull();
    expect(document.querySelector('.task-creation')).not.toBeNull();
    expect(progress.element.querySelector('.task-creation-title')?.textContent).toBe(
      'Adding: repo-a',
    );
    handle.close();
  });
});
