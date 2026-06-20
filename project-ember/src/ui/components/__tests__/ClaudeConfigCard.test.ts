import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createClaudeConfigCard } from '../ClaudeConfigCard';
import { invoke } from '../../../invoke';

vi.mock('../../../invoke', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

const SETTINGS = {
  stored: { 'opus-model': 'claude-fable-5', 'haiku-model': null },
  resolved: {
    'opus-model': 'claude-fable-5',
    'sonnet-model': 'claude-opus-4-8',
    'haiku-model': null,
    effort: 'xhigh',
  },
};

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === 'get_claude_settings') return SETTINGS;
    if (cmd === 'set_claude_setting') {
      // Mirror the backend's accepted vocabulary so a wrong state fails the test.
      const state = (args as { state?: string })?.state;
      if (state !== 'value' && state !== 'none' && state !== 'default') {
        throw new Error(`unknown state: ${state}`);
      }
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });
});

async function mountCard(profilePath: string | null = null): Promise<HTMLElement> {
  const card = createClaudeConfigCard({ profilePath });
  await flush();
  return card;
}

function row(card: HTMLElement, index: number): HTMLElement {
  return card.querySelectorAll<HTMLElement>('.claude-config-row')[index];
}

function activeSegment(rowEl: HTMLElement): string {
  return rowEl.querySelector('.segmented-control-active')?.textContent ?? '';
}

function clickSegment(rowEl: HTMLElement, label: string): void {
  const btn = Array.from(rowEl.querySelectorAll<HTMLButtonElement>('.segmented-control-btn')).find(
    (b) => b.textContent === label,
  );
  btn!.click();
}

function lastSetCall(): Record<string, unknown> | undefined {
  const calls = invokeMock.mock.calls.filter((c) => c[0] === 'set_claude_setting');
  return calls.at(-1)?.[1] as Record<string, unknown> | undefined;
}

describe('createClaudeConfigCard', () => {
  it('renders one row per setting and loads from get_claude_settings', async () => {
    const card = await mountCard();
    expect(card.querySelectorAll('.claude-config-row').length).toBe(4);
    expect(invokeMock).toHaveBeenCalledWith('get_claude_settings', { profilePath: null });
  });

  it('maps stored value / null / absent to the Custom / None / Default segment', async () => {
    const card = await mountCard();
    expect(activeSegment(row(card, 0))).toBe('Custom'); // opus = stored string
    expect(activeSegment(row(card, 1))).toBe('Default'); // sonnet absent
    expect(activeSegment(row(card, 2))).toBe('None'); // haiku = null
    expect(activeSegment(row(card, 3))).toBe('Default'); // effort absent
  });

  it('shows the stored value in the Custom input, and resolved values in Default/None hints', async () => {
    const card = await mountCard();
    expect(row(card, 0).querySelector<HTMLInputElement>('.claude-config-input')!.value).toBe(
      'claude-fable-5',
    );
    expect(row(card, 1).querySelector('.claude-config-hint')!.textContent).toContain(
      'claude-opus-4-8',
    );
    expect(row(card, 2).querySelector('.claude-config-hint')!.textContent).toContain(
      'Claude Code decides',
    );
    expect(row(card, 3).querySelector('.claude-config-hint')!.textContent).toContain('xhigh');
  });

  it('persists Default and None segment clicks', async () => {
    const card = await mountCard();

    clickSegment(row(card, 0), 'Default');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'opus-model',
      state: 'default',
      value: null,
    });

    clickSegment(row(card, 0), 'None');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'opus-model',
      state: 'none',
      value: null,
    });
  });

  it('switching to Custom seeds the input from the resolved value without persisting', async () => {
    const card = await mountCard();
    clickSegment(row(card, 1), 'Custom'); // sonnet
    const input = row(card, 1).querySelector<HTMLInputElement>('.claude-config-input')!;
    expect(input.value).toBe('claude-opus-4-8'); // seeded from resolved
    expect(invokeMock.mock.calls.some((c) => c[0] === 'set_claude_setting')).toBe(false);
  });

  it('commits a typed model id on Enter', async () => {
    const card = await mountCard();
    clickSegment(row(card, 1), 'Custom');
    const input = row(card, 1).querySelector<HTMLInputElement>('.claude-config-input')!;
    input.value = 'claude-opus-4-7';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'sonnet-model',
      state: 'value',
      value: 'claude-opus-4-7',
    });
  });

  it('commits a suggestion chip click', async () => {
    const card = await mountCard();
    const chip = row(card, 0).querySelector<HTMLButtonElement>('.claude-config-chip')!;
    const expected = chip.textContent;
    chip.click();
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'opus-model',
      state: 'value',
      value: expected,
    });
  });

  it('maps a cleared input to Default rather than an empty value', async () => {
    const card = await mountCard();
    const input = row(card, 0).querySelector<HTMLInputElement>('.claude-config-input')!;
    input.value = '   ';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'opus-model',
      state: 'default',
      value: null,
    });
  });

  it('commits an effort level chosen from the dropdown', async () => {
    const card = await mountCard();
    clickSegment(row(card, 3), 'Custom'); // effort
    row(card, 3).querySelector<HTMLButtonElement>('.dropdown-trigger')!.click();
    const option = Array.from(
      row(card, 3).querySelectorAll<HTMLButtonElement>('.dropdown-item-option'),
    ).find((b) => b.textContent?.includes('max'));
    option!.click();
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'effort',
      state: 'value',
      value: 'max',
    });
  });

  it('passes the profile path through to both commands', async () => {
    const card = await mountCard('/home/me/devora');
    expect(invokeMock).toHaveBeenCalledWith('get_claude_settings', {
      profilePath: '/home/me/devora',
    });
    clickSegment(row(card, 0), 'None');
    await flush();
    expect(lastSetCall()).toMatchObject({ profilePath: '/home/me/devora' });
  });
});
