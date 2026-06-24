/**
 * Asana API token editor - a keychain-backed secret (service `devora-asana`), NOT a config.json key.
 * Shows whether a token is stored and lets the user save or clear it via the keychain commands (`get_asana_token_status` / `set_asana_token` / `clear_asana_token`).
 * Self-loads its status on mount and refreshes after each change.
 * DOM: `div.asana-token-row`.
 */

import { invoke } from '../../invoke';
import { createStatusDot } from './StatusDot';

export function createAsanaTokenRow(): HTMLElement {
  const row = document.createElement('div');
  row.className = 'asana-token-row';

  const label = document.createElement('div');
  label.className = 'config-row-label';
  const name = document.createElement('div');
  name.className = 'config-row-name';
  name.textContent = 'API token';
  label.appendChild(name);
  const status = document.createElement('div');
  status.className = 'asana-token-status';
  label.appendChild(status);
  row.appendChild(label);

  const controls = document.createElement('div');
  controls.className = 'asana-token-controls';

  const input = document.createElement('input');
  input.type = 'password';
  input.className = 'config-input';
  input.placeholder = 'paste token, then Save';
  controls.appendChild(input);

  const saveBtn = document.createElement('button');
  saveBtn.className = 'asana-token-btn';
  saveBtn.textContent = 'Save';
  controls.appendChild(saveBtn);

  const clearBtn = document.createElement('button');
  clearBtn.className = 'asana-token-btn asana-token-btn-secondary';
  clearBtn.textContent = 'Clear';
  controls.appendChild(clearBtn);

  const note = document.createElement('div');
  note.className = 'config-row-note';
  note.textContent = 'Stored in your OS keychain (service devora-asana), shared with debi.';
  controls.appendChild(note);

  row.appendChild(controls);

  const renderStatus = (present: boolean): void => {
    status.innerHTML = '';
    status.appendChild(createStatusDot(present ? 'clean' : 'pending'));
    const text = document.createElement('span');
    text.textContent = present ? 'Set' : 'Not set';
    status.appendChild(text);
    clearBtn.disabled = !present;
  };

  const refresh = async (): Promise<void> => {
    try {
      renderStatus(await invoke<boolean>('get_asana_token_status'));
    } catch {
      // invoke already surfaced the error
    }
  };

  const save = async (): Promise<void> => {
    const token = input.value.trim();
    if (token === '') return;
    try {
      await invoke('set_asana_token', { token });
      input.value = '';
      await refresh();
    } catch {
      // invoke already surfaced the error
    }
  };

  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      void save();
    }
  });
  saveBtn.addEventListener('click', () => void save());
  clearBtn.addEventListener('click', async () => {
    try {
      await invoke('clear_asana_token');
      input.value = '';
      await refresh();
    } catch {
      // invoke already surfaced the error
    }
  });

  void refresh();
  return row;
}
