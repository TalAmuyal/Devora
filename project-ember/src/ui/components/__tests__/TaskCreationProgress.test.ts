import { describe, it, expect, vi } from 'vitest';
import { createTaskCreationProgress } from '../TaskCreationProgress';

describe('createTaskCreationProgress', () => {
  it('renders a title with the task name', () => {
    const { element } = createTaskCreationProgress('Fix login bug');
    const title = element.querySelector('.task-creation-title');
    expect(title?.textContent).toBe('Creating: Fix login bug');
  });

  it('adds an active step with a spinner', () => {
    const handle = createTaskCreationProgress('T');
    handle.setStep('Fetching repo-a');

    const steps = handle.element.querySelectorAll('.task-creation-step');
    expect(steps.length).toBe(1);
    expect(steps[0].querySelector('.task-creation-spinner')).not.toBeNull();
    expect(steps[0].classList.contains('task-creation-step-active')).toBe(true);
    expect(steps[0].textContent).toContain('Fetching repo-a');
  });

  it('marks the previous step done when a new step starts', () => {
    const handle = createTaskCreationProgress('T');
    handle.setStep('Fetching repo-a');
    handle.setStep('Creating worktree repo-a');

    const steps = handle.element.querySelectorAll('.task-creation-step');
    expect(steps.length).toBe(2);

    // First step: done (green status dot, no spinner, not active)
    expect(steps[0].querySelector('.task-creation-spinner')).toBeNull();
    expect(steps[0].querySelector('.status-dot.clean')).not.toBeNull();
    expect(steps[0].classList.contains('task-creation-step-active')).toBe(false);

    // Second step: active
    expect(steps[1].querySelector('.task-creation-spinner')).not.toBeNull();
    expect(steps[1].classList.contains('task-creation-step-active')).toBe(true);
  });

  it('appends log lines in order', () => {
    const handle = createTaskCreationProgress('T');
    handle.appendLog('line one');
    handle.appendLog('line two');

    const lines = handle.element.querySelectorAll('.task-creation-log-line');
    expect(lines.length).toBe(2);
    expect(lines[0].textContent).toBe('line one');
    expect(lines[1].textContent).toBe('line two');
  });

  it('caps the log at 500 lines, dropping the oldest', () => {
    const handle = createTaskCreationProgress('T');
    for (let i = 0; i < 520; i++) {
      handle.appendLog(`line ${i}`);
    }
    const lines = handle.element.querySelectorAll('.task-creation-log-line');
    expect(lines.length).toBe(500);
    expect(lines[0].textContent).toBe('line 20');
    expect(lines[lines.length - 1].textContent).toBe('line 519');
  });

  it('cancel button fires the cancel callback while running', () => {
    const handle = createTaskCreationProgress('T');
    const onCancel = vi.fn();
    handle.onCancel(onCancel);

    const button = handle.element.querySelector<HTMLButtonElement>('.task-creation-action')!;
    expect(button.textContent).toBe('Cancel');
    button.click();
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('showError marks the active step failed and switches the action to Close', () => {
    const handle = createTaskCreationProgress('T');
    const onCancel = vi.fn();
    const onClose = vi.fn();
    handle.onCancel(onCancel);
    handle.onClose(onClose);

    handle.setStep('Fetching repo-a');
    handle.showError('git fetch failed for repo-a (see log)');

    const step = handle.element.querySelector('.task-creation-step')!;
    expect(step.querySelector('.status-dot.error')).not.toBeNull();

    const error = handle.element.querySelector('.task-creation-error') as HTMLElement;
    expect(error.hidden).toBe(false);
    expect(error.textContent).toBe('git fetch failed for repo-a (see log)');

    const button = handle.element.querySelector<HTMLButtonElement>('.task-creation-action')!;
    expect(button.textContent).toBe('Close');

    button.click();
    expect(onClose).toHaveBeenCalledOnce();
    expect(onCancel).not.toHaveBeenCalled();
  });
});
