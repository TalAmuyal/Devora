import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createTaskTrackerCard } from '../TaskTrackerCard';
import { invoke } from '../../../invoke';

vi.mock('../../../invoke', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

let provider: string | null;
let tokenPresent: boolean;

beforeEach(() => {
  provider = null;
  tokenPresent = false;
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_config_settings') {
      return {
        stored: provider === null ? {} : { 'task-tracker.provider': provider },
        resolved: { 'task-tracker.provider': provider },
      };
    }
    if (cmd === 'set_config_setting') return null;
    if (cmd === 'get_asana_token_status') return tokenPresent;
    if (cmd === 'set_asana_token') return null;
    if (cmd === 'clear_asana_token') return null;
    throw new Error(`unexpected command ${cmd}`);
  });
});

describe('createTaskTrackerCard', () => {
  it('hides the Asana fields and token editor when the provider is not Asana', async () => {
    const card = createTaskTrackerCard(null);
    await flush();
    // Only the provider row is visible.
    expect(card.querySelectorAll('.config-row')).toHaveLength(1);
    expect(card.querySelector('.asana-token-row')).toBeNull();
  });

  it('shows the Asana fields and token editor when the provider resolves to Asana', async () => {
    provider = 'asana';
    const card = createTaskTrackerCard(null);
    await flush();
    // provider + 4 Asana ID fields.
    expect(card.querySelectorAll('.config-row')).toHaveLength(5);
    expect(card.querySelector('.asana-token-row')).not.toBeNull();
  });

  it('reflects keychain token status and saves a new token', async () => {
    provider = 'asana';
    tokenPresent = false;
    const card = createTaskTrackerCard(null);
    await flush();

    const status = card.querySelector('.asana-token-status')?.textContent ?? '';
    expect(status).toContain('Not set');

    const input = card.querySelector<HTMLInputElement>('.asana-token-row .config-input');
    input!.value = '1/secret-token';
    const saveBtn = Array.from(card.querySelectorAll<HTMLButtonElement>('.asana-token-btn')).find(
      (b) => b.textContent === 'Save',
    );
    saveBtn!.click();
    await flush();

    const setCall = invokeMock.mock.calls.find((c) => c[0] === 'set_asana_token');
    expect(setCall?.[1]).toMatchObject({ token: '1/secret-token' });
  });
});
