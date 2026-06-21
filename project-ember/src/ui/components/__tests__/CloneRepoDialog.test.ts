import { describe, it, expect, vi, afterEach } from 'vitest';
import { showCloneRepoDialog } from '../CloneRepoDialog';

afterEach(() => {
  document.querySelectorAll('.clone-repo-dialog-backdrop').forEach((el) => el.remove());
});

function backdrop(): HTMLElement | null {
  return document.querySelector('.clone-repo-dialog-backdrop');
}

function urlInput(): HTMLInputElement {
  return document.querySelector('.clone-repo-dialog-input') as HTMLInputElement;
}

function hint(): HTMLElement {
  return document.querySelector('.clone-repo-dialog-hint') as HTMLElement;
}

describe('showCloneRepoDialog', () => {
  it('renders the form focused on the URL input', () => {
    const handle = showCloneRepoDialog({ profilePath: '/home/me/work' });
    expect(backdrop()).not.toBeNull();
    expect(document.activeElement).toBe(urlInput());
    handle.close();
  });

  it('shows a hint with the resolved destination as the user types', () => {
    const handle = showCloneRepoDialog({ profilePath: '/home/me/work' });
    expect(hint().hidden).toBe(true);

    urlInput().value = 'git@github.com:org/repo.git';
    urlInput().dispatchEvent(new Event('input'));
    expect(hint().hidden).toBe(false);
    expect(hint().textContent).toContain('/home/me/work/repos/repo');

    urlInput().value = 'not a url';
    urlInput().dispatchEvent(new Event('input'));
    expect(hint().hidden).toBe(true);
    handle.close();
  });

  it('submits the trimmed URL', () => {
    const handle = showCloneRepoDialog({ profilePath: '/home/me/work' });
    const onSubmit = vi.fn();
    handle.onSubmit(onSubmit);
    urlInput().value = '  https://github.com/org/repo  ';
    document.querySelector<HTMLButtonElement>('.clone-repo-dialog-clone')!.click();
    expect(onSubmit).toHaveBeenCalledWith({ url: 'https://github.com/org/repo' });
    handle.close();
  });

  it('does not submit an empty URL', () => {
    const handle = showCloneRepoDialog({ profilePath: '/home/me/work' });
    const onSubmit = vi.fn();
    handle.onSubmit(onSubmit);
    document.querySelector<HTMLButtonElement>('.clone-repo-dialog-clone')!.click();
    expect(onSubmit).not.toHaveBeenCalled();
    handle.close();
  });

  it('cancel fires onCancel and removes the dialog', () => {
    const handle = showCloneRepoDialog({ profilePath: '/home/me/work' });
    const onCancel = vi.fn();
    handle.onCancel(onCancel);
    document.querySelector<HTMLButtonElement>('.clone-repo-dialog-cancel')!.click();
    expect(onCancel).toHaveBeenCalledOnce();
    expect(backdrop()).toBeNull();
  });

  it('showProgress swaps the form for a progress view', () => {
    const handle = showCloneRepoDialog({ profilePath: '/home/me/work' });
    handle.showProgress('Cloning: repo');
    expect(document.querySelector('.clone-repo-dialog-input')).toBeNull();
    expect(document.querySelector('.clone-repo-dialog .task-creation')).not.toBeNull();
    handle.close();
  });
});
