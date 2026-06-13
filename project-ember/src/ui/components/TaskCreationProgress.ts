/**
 * Progress UI for an in-flight task creation, shown as a panel overlay on the new session tab.
 * Renders the high-level steps (with per-step state) plus an expandable live log of subprocess output, and a footer action that cancels while running or closes after a failure.
 *
 * DOM: `div.task-creation`.
 * Driven by the backend creation channel via the returned handle.
 */

import { createStatusDot } from './StatusDot';
import { createKeyboardHintBar } from './KeyboardHintBar';

export interface TaskCreationProgressHandle {
  element: HTMLElement;
  /** Mark the current step done and begin a new active step. */
  setStep(label: string): void;
  /** Append a line of subprocess output to the live log. */
  appendLog(line: string): void;
  /** Mark the active step failed and surface the error, switching the footer action to "Close". */
  showError(message: string): void;
  /** Footer "Cancel" (while running) and Esc/q dismissal route here. */
  onCancel(callback: () => void): void;
  /** Footer "Close" (after a failure) and Esc/q dismissal route here. */
  onClose(callback: () => void): void;
}

type StepState = 'active' | 'done' | 'failed';

/** Cap retained log lines so a chatty prepare-command can't grow the DOM unbounded. */
const MAX_LOG_LINES = 500;

export function createTaskCreationProgress(taskName: string): TaskCreationProgressHandle {
  const element = document.createElement('div');
  element.className = 'task-creation';

  const title = document.createElement('h2');
  title.className = 'task-creation-title';
  title.textContent = `Creating: ${taskName}`;
  element.appendChild(title);

  const stepsList = document.createElement('ol');
  stepsList.className = 'task-creation-steps';
  element.appendChild(stepsList);

  const log = document.createElement('details');
  log.className = 'task-creation-log';
  const logSummary = document.createElement('summary');
  logSummary.textContent = 'Output';
  log.appendChild(logSummary);
  const logBody = document.createElement('div');
  logBody.className = 'task-creation-log-body';
  log.appendChild(logBody);
  element.appendChild(log);

  const errorEl = document.createElement('div');
  errorEl.className = 'task-creation-error';
  errorEl.hidden = true;
  element.appendChild(errorEl);

  const footer = document.createElement('div');
  footer.className = 'task-creation-footer';
  footer.appendChild(createKeyboardHintBar({ hints: [{ keys: 'Esc', description: 'dismiss' }] }));
  const actionBtn = document.createElement('button');
  actionBtn.className = 'task-creation-action';
  actionBtn.textContent = 'Cancel';
  footer.appendChild(actionBtn);
  element.appendChild(footer);

  let cancelCallback: (() => void) | null = null;
  let closeCallback: (() => void) | null = null;
  let failed = false;

  actionBtn.addEventListener('click', () => {
    if (failed) {
      closeCallback?.();
    } else {
      cancelCallback?.();
    }
  });

  let activeRow: { setState: (state: StepState) => void } | null = null;

  function indicatorFor(state: StepState): HTMLElement {
    if (state === 'active') {
      const spinner = document.createElement('div');
      spinner.className = 'task-creation-spinner';
      return spinner;
    }
    return createStatusDot(state === 'done' ? 'clean' : 'error');
  }

  function setStep(label: string): void {
    activeRow?.setState('done');

    const row = document.createElement('li');
    row.className = 'task-creation-step';
    let indicator = indicatorFor('active');
    const labelEl = document.createElement('span');
    labelEl.className = 'task-creation-step-label';
    labelEl.textContent = label;
    row.appendChild(indicator);
    row.appendChild(labelEl);
    stepsList.appendChild(row);

    const setState = (state: StepState): void => {
      const next = indicatorFor(state);
      row.replaceChild(next, indicator);
      indicator = next;
      row.classList.toggle('task-creation-step-active', state === 'active');
    };
    setState('active');
    activeRow = { setState };
  }

  function appendLog(line: string): void {
    const lineEl = document.createElement('div');
    lineEl.className = 'task-creation-log-line';
    lineEl.textContent = line;
    logBody.appendChild(lineEl);
    while (logBody.childElementCount > MAX_LOG_LINES) {
      logBody.firstElementChild?.remove();
    }
    logBody.scrollTop = logBody.scrollHeight;
  }

  function showError(message: string): void {
    activeRow?.setState('failed');
    activeRow = null;
    errorEl.textContent = message;
    errorEl.hidden = false;
    failed = true;
    actionBtn.textContent = 'Close';
    // Reveal the output: it almost always explains the failure.
    log.open = true;
  }

  return {
    element,
    setStep,
    appendLog,
    showError,
    onCancel: (callback) => {
      cancelCallback = callback;
    },
    onClose: (callback) => {
      closeCallback = callback;
    },
  };
}
