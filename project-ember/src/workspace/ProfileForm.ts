/**
 * Profile creation/registration form: Name + Root Path (with native folder picker) and live path validation driving three states — new directory, existing profile detected (name locks, button relabels to "Register Profile"), and invalid path.
 * Used inline in the Settings Hub's detail panel and inside the first-run welcome card.
 * DOM: `div.pm-form`.
 */

import { open } from '@tauri-apps/plugin-dialog';
import { invoke } from '../invoke';
import { showError } from '../errors';

export const PATH_VALIDATION_DEBOUNCE_MS = 250;

export interface RegisteredProfile {
  name: string;
  path: string;
}

interface ProfilePathValidation {
  kind: 'new' | 'existing_profile' | 'invalid';
  name: string | null;
  error: string | null;
  expandedPath: string;
}

export interface ProfileFormOptions {
  initialPath?: string;
  /** Called after `register_profile` succeeds. */
  onRegistered: (profile: RegisteredProfile) => void;
  /** Renders a Cancel button when provided (omitted on the first-run welcome). */
  onCancel?: () => void;
}

export interface ProfileFormHandle {
  element: HTMLElement;
  focus(): void;
}

export function createProfileForm(options: ProfileFormOptions): ProfileFormHandle {
  let latestValidation: ProfilePathValidation | null = null;
  let validationPending = false;
  let validationSeq = 0;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  // What the user actually typed, preserved across the existing-profile state overwriting the visible name input with the detected name.
  let typedName = '';

  const form = document.createElement('div');
  form.className = 'pm-form';

  // -- Name field --
  const nameField = document.createElement('div');
  nameField.className = 'pm-form-field';
  const nameLabel = document.createElement('label');
  nameLabel.className = 'pm-form-label';
  nameLabel.textContent = 'Name';
  nameField.appendChild(nameLabel);
  const nameInput = document.createElement('input');
  nameInput.type = 'text';
  nameInput.className = 'pm-form-input pm-form-name';
  nameInput.placeholder = 'e.g. Personal';
  nameField.appendChild(nameInput);
  form.appendChild(nameField);

  // -- Path field --
  const pathField = document.createElement('div');
  pathField.className = 'pm-form-field';
  const pathLabel = document.createElement('label');
  pathLabel.className = 'pm-form-label';
  pathLabel.textContent = 'Root Path';
  pathField.appendChild(pathLabel);

  const pathRow = document.createElement('div');
  pathRow.className = 'pm-form-path-row';
  const pathInput = document.createElement('input');
  pathInput.type = 'text';
  pathInput.className = 'pm-form-input pm-form-path';
  pathInput.placeholder = '~/devora';
  pathRow.appendChild(pathInput);
  const browseBtn = document.createElement('button');
  browseBtn.className = 'pm-form-browse';
  browseBtn.textContent = 'Browse…';
  pathRow.appendChild(browseBtn);
  pathField.appendChild(pathRow);

  const statusEl = document.createElement('div');
  statusEl.className = 'pm-form-status';
  statusEl.style.display = 'none';
  pathField.appendChild(statusEl);
  form.appendChild(pathField);

  // -- Info box: what gets created --
  const info = document.createElement('div');
  info.className = 'pm-form-info';
  const infoTitle = document.createElement('div');
  infoTitle.className = 'pm-form-info-title';
  infoTitle.textContent = 'Inside the profile root:';
  info.appendChild(infoTitle);
  const infoList = document.createElement('ul');
  for (const [entry, desc] of [
    ['config.json', 'profile-specific configuration'],
    ['repos/', 'clone your git repos here (worktrees are created from them)'],
    ['workspaces/', 'Devora-managed worktrees, one directory per workspace'],
  ]) {
    const li = document.createElement('li');
    const key = document.createElement('span');
    key.className = 'pm-form-info-key';
    key.textContent = entry;
    li.appendChild(key);
    li.appendChild(document.createTextNode(` — ${desc}`));
    infoList.appendChild(li);
  }
  info.appendChild(infoList);
  form.appendChild(info);

  // -- Actions --
  const actions = document.createElement('div');
  actions.className = 'pm-form-actions';
  const submitBtn = document.createElement('button');
  submitBtn.className = 'pm-form-submit';
  submitBtn.textContent = 'Create Profile';
  submitBtn.disabled = true;
  actions.appendChild(submitBtn);
  if (options.onCancel) {
    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'pm-form-cancel';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => options.onCancel!());
    actions.appendChild(cancelBtn);
  }
  form.appendChild(actions);

  // -- Behavior --

  const updateSubmitEnabled = (): void => {
    const v = latestValidation;
    const ready =
      !validationPending &&
      v !== null &&
      (v.kind === 'existing_profile' ||
        (v.kind === 'new' && nameInput.value.trim() !== ''));
    submitBtn.disabled = !ready;
  };

  const applyValidation = (v: ProfilePathValidation): void => {
    latestValidation = v;
    statusEl.style.display = '';
    statusEl.classList.remove('ok', 'detect', 'err');
    if (v.kind === 'new') {
      statusEl.classList.add('ok');
      statusEl.textContent = '✓ New profile — the structure below will be created';
    } else if (v.kind === 'existing_profile') {
      statusEl.classList.add('detect');
      statusEl.textContent = `◆ Existing profile "${v.name}" found — it will be registered as-is`;
    } else {
      statusEl.classList.add('err');
      statusEl.textContent = `✗ ${v.error}`;
    }

    if (v.kind === 'existing_profile') {
      nameInput.value = v.name ?? '';
      nameInput.disabled = true;
      submitBtn.textContent = 'Register Profile';
    } else {
      if (nameInput.disabled) {
        nameInput.value = typedName;
      }
      nameInput.disabled = false;
      submitBtn.textContent = 'Create Profile';
    }
    updateSubmitEnabled();
  };

  const runValidation = async (): Promise<void> => {
    const seq = ++validationSeq;
    const path = pathInput.value;
    if (path.trim() === '') {
      // Don't scold an untouched/cleared field; just keep submit disabled.
      latestValidation = null;
      validationPending = false;
      statusEl.style.display = 'none';
      updateSubmitEnabled();
      return;
    }
    try {
      const v = await invoke<ProfilePathValidation>('validate_profile_path', { path });
      if (seq !== validationSeq) return;
      validationPending = false;
      applyValidation(v);
    } catch (_) {
      // invoke already surfaced the error; keep submit disabled
      if (seq !== validationSeq) return;
      validationPending = false;
      latestValidation = null;
      updateSubmitEnabled();
    }
  };

  const scheduleValidation = (): void => {
    validationPending = true;
    updateSubmitEnabled();
    if (debounceTimer !== null) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void runValidation(), PATH_VALIDATION_DEBOUNCE_MS);
  };

  const submit = async (): Promise<void> => {
    if (submitBtn.disabled) return;
    submitBtn.disabled = true;
    try {
      const profile = await invoke<RegisteredProfile>('register_profile', {
        path: pathInput.value,
        name: nameInput.value.trim(),
      });
      options.onRegistered(profile);
    } catch (_) {
      // invoke already surfaced the error; the form stays open
      updateSubmitEnabled();
    }
  };

  const handleInputKeydown = (e: KeyboardEvent): void => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopImmediatePropagation();
      void submit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopImmediatePropagation();
      (e.target as HTMLInputElement).blur();
    }
  };

  pathInput.addEventListener('input', scheduleValidation);
  pathInput.addEventListener('keydown', handleInputKeydown);
  nameInput.addEventListener('input', () => {
    typedName = nameInput.value;
    updateSubmitEnabled();
  });
  nameInput.addEventListener('keydown', handleInputKeydown);
  submitBtn.addEventListener('click', () => void submit());

  browseBtn.addEventListener('click', async () => {
    try {
      const dir = await open({ directory: true });
      if (typeof dir === 'string') {
        pathInput.value = dir;
        void runValidation();
      }
    } catch (e) {
      showError(`Folder picker failed: ${e}`);
    }
  });

  if (options.initialPath) {
    pathInput.value = options.initialPath;
    validationPending = true;
    void runValidation();
  }

  return {
    element: form,
    focus() {
      nameInput.focus();
    },
  };
}
