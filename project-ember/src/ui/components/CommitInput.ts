/**
 * A text input paired with a Save button, for the Settings Hub reveal-inputs (ConfigCard text fields, ClaudeConfigCard model tiers).
 * The Save button makes the otherwise-hidden commit gesture discoverable: it appears only while the input holds a non-empty value that differs from what's saved at this scope, and both it and Enter commit through the same callback.
 * Showing it only for a real, differing value keeps it out of the way for a saved value at rest and stops it from becoming a "save" that silently reverts to Default on an empty field (that stays on the Default segment / Enter).
 * DOM: `div.config-input-wrap > input.config-input + button.config-save`.
 */

import { blurOnEscape } from '../focus';

export interface CommitInputHandle {
  /** The wrapper holding the input and its Save button; append this where the input should go. */
  readonly root: HTMLElement;
  /** The text input, exposed for focus management (e.g. `editor.focusOnRender`). */
  readonly input: HTMLInputElement;
}

export interface CommitInputOptions {
  placeholder?: string;
  /** Initial text shown in the input. */
  value: string;
  /** The value currently persisted at this scope ('' when none); the Save button shows only while the input differs from it. */
  savedValue: string;
  /** Persist the input's current text; wired to both Enter and the Save button. */
  onCommit: (raw: string) => void;
}

export function createCommitInput(options: CommitInputOptions): CommitInputHandle {
  const root = document.createElement('div');
  root.className = 'config-input-wrap';

  const input = document.createElement('input');
  input.type = 'text';
  input.className = 'config-input';
  if (options.placeholder) input.placeholder = options.placeholder;
  input.value = options.value;

  const save = document.createElement('button');
  save.type = 'button';
  save.className = 'config-save';
  save.textContent = '💾';
  save.title = 'Save';
  save.setAttribute('aria-label', 'Save');

  const syncSave = (): void => {
    const trimmed = input.value.trim();
    save.hidden = trimmed === '' || trimmed === options.savedValue;
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      options.onCommit(input.value);
    }
  });
  input.addEventListener('input', syncSave);
  blurOnEscape(input);

  // Commit without first pulling focus off the input, so a save-by-click leaves focus where the user expects.
  save.addEventListener('mousedown', (e) => e.preventDefault());
  save.addEventListener('click', () => options.onCommit(input.value));

  syncSave();

  root.appendChild(input);
  root.appendChild(save);
  return { root, input };
}
