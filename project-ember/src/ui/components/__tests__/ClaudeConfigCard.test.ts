import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createClaudeConfigCard } from '../ClaudeConfigCard';
import { invoke, invokeLogOnly } from '../../../invoke';

vi.mock('../../../invoke', () => ({ invoke: vi.fn(), invokeLogOnly: vi.fn() }));
const invokeMock = vi.mocked(invoke);
const invokeLogOnlyMock = vi.mocked(invokeLogOnly);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

const SETTINGS = {
  choices: {
    effort_levels: ['low', 'medium', 'high', 'xhigh', 'max'],
    permission_mode_suggestions: ['plan', 'acceptEdits', 'auto'],
  },
  stored: { 'opus-model': 'claude-fable-5', 'haiku-model': null },
  resolved: {
    command: 'claude',
    'opus-model': 'claude-fable-5',
    'sonnet-model': 'claude-opus-4-8',
    'haiku-model': null,
    effort: 'xhigh',
    'default-model': 'opusplan',
    'permission-mode': 'plan',
  },
};

const MODELS = [
  { id: 'claude-opus-5-5', name: 'Opus 5.5' },
  { id: 'claude-sonnet-5-5', name: 'Sonnet 5.5' },
  { id: 'claude-opus-4-8', name: 'Opus 4.8' },
];

beforeEach(() => {
  invokeLogOnlyMock.mockReset();
  invokeLogOnlyMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_claude_model_options') return MODELS;
    throw new Error(`unexpected command ${cmd}`);
  });
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

afterEach(() => {
  document.body.innerHTML = '';
});

async function mountCard(profilePath: string | null = null): Promise<HTMLElement> {
  const card = createClaudeConfigCard({ profilePath, modelProfilePath: '/profiles/work' });
  await flush();
  return card;
}

function row(card: HTMLElement, index: number): HTMLElement {
  return card.querySelectorAll<HTMLElement>('.config-row')[index];
}

function rowByLabel(card: HTMLElement, label: string): HTMLElement {
  const found = Array.from(card.querySelectorAll<HTMLElement>('.config-row')).find(
    (r) => r.querySelector('.config-row-name')?.textContent === label,
  );
  if (!found) throw new Error(`row not found: ${label}`);
  return found;
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
  it('renders effort and permission suggestions from backend metadata, and no model chips', async () => {
    invokeMock.mockResolvedValue({
      ...SETTINGS,
      choices: { effort_levels: ['next-effort'], permission_mode_suggestions: ['next-permission'] },
    });
    const card = await mountCard();
    expect(row(card, 1).querySelectorAll('.config-chip').length).toBe(0);
    expect(row(card, 4).textContent).toContain('next-effort');
    clickSegment(row(card, 6), 'Custom');
    expect(row(card, 6).textContent).toContain('next-permission');
  });

  it('renders one row per setting and loads from get_claude_settings', async () => {
    const card = await mountCard();
    expect(card.querySelectorAll('.config-row').length).toBe(7);
    expect(invokeMock).toHaveBeenCalledWith('get_claude_settings', { profilePath: null });
  });

  it('maps stored value / null / absent to the Custom / None / Default segment', async () => {
    const card = await mountCard();
    expect(activeSegment(row(card, 1))).toBe('Custom'); // opus = stored string
    expect(activeSegment(row(card, 2))).toBe('Default'); // sonnet absent
    expect(activeSegment(row(card, 3))).toBe('None'); // haiku = null
    expect(activeSegment(row(card, 4))).toBe('Default'); // effort absent
  });

  it('shows the stored value in the Custom input, and resolved values in Default/None hints', async () => {
    const card = await mountCard();
    expect(row(card, 1).querySelector<HTMLInputElement>('.config-input')!.value).toBe(
      'claude-fable-5',
    );
    expect(row(card, 2).querySelector('.config-hint')!.textContent).toContain(
      'claude-opus-4-8',
    );
    expect(row(card, 3).querySelector('.config-hint')!.textContent).toContain(
      'Claude Code decides',
    );
    expect(row(card, 4).querySelector('.config-hint')!.textContent).toContain('xhigh');
  });

  it('persists Default and None segment clicks', async () => {
    const card = await mountCard();

    clickSegment(row(card, 1), 'Default');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'opus-model',
      state: 'default',
      value: null,
    });

    clickSegment(row(card, 1), 'None');
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
    clickSegment(row(card, 2), 'Custom'); // sonnet
    const input = row(card, 2).querySelector<HTMLInputElement>('.config-input')!;
    expect(input.value).toBe('claude-opus-4-8'); // seeded from resolved
    expect(invokeMock.mock.calls.some((c) => c[0] === 'set_claude_setting')).toBe(false);
  });

  it('commits a typed model id on Enter', async () => {
    const card = await mountCard();
    clickSegment(row(card, 2), 'Custom');
    const input = row(card, 2).querySelector<HTMLInputElement>('.config-input')!;
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

  it('hides the Save button for a stored value at rest and shows it after an edit', async () => {
    const card = await mountCard();
    const opusInput = row(card, 1).querySelector<HTMLInputElement>('.config-input')!;
    const save = () => row(card, 1).querySelector<HTMLButtonElement>('.config-save')!;
    // opus is stored as 'claude-fable-5' -> input matches the saved value -> nothing to save.
    expect(save().hidden).toBe(true);
    opusInput.value = 'claude-opus-4-7';
    opusInput.dispatchEvent(new Event('input', { bubbles: true }));
    expect(save().hidden).toBe(false);
  });

  it('commits a typed model id when Save is clicked', async () => {
    const card = await mountCard();
    clickSegment(row(card, 2), 'Custom'); // sonnet
    const input = row(card, 2).querySelector<HTMLInputElement>('.config-input')!;
    input.value = 'claude-opus-4-7';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    row(card, 2).querySelector<HTMLButtonElement>('.config-save')!.click();
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'sonnet-model',
      state: 'value',
      value: 'claude-opus-4-7',
    });
  });

  it('maps a cleared input to Default rather than an empty value', async () => {
    const card = await mountCard();
    const input = row(card, 1).querySelector<HTMLInputElement>('.config-input')!;
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

  it('renders the effort segments as Default, levels high→low, then None', async () => {
    const card = await mountCard();
    const labels = Array.from(
      row(card, 4).querySelectorAll<HTMLButtonElement>('.segmented-control-btn'),
    ).map((b) => b.textContent);
    expect(labels).toEqual(['Default', 'max', 'xhigh', 'high', 'medium', 'low', 'None']);
  });

  it('commits an effort level chosen from the segmented control', async () => {
    const card = await mountCard();
    clickSegment(row(card, 4), 'max'); // effort
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'effort',
      state: 'value',
      value: 'max',
    });
  });

  it('commits None and Default from the effort segmented control', async () => {
    const card = await mountCard();

    clickSegment(row(card, 4), 'None');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'effort',
      state: 'none',
      value: null,
    });

    clickSegment(row(card, 4), 'Default');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'effort',
      state: 'default',
      value: null,
    });
  });

  it('renders the Default model as an On/Off toggle, On when resolved to opusplan', async () => {
    const card = await mountCard();
    const toggle = rowByLabel(card, 'Default model');
    const labels = Array.from(
      toggle.querySelectorAll<HTMLButtonElement>('.segmented-control-btn'),
    ).map((b) => b.textContent);
    expect(labels).toEqual(['On', 'Off']);
    expect(activeSegment(toggle)).toBe('On');
  });

  it('persists the Default model toggle as opusplan (On) or none (Off)', async () => {
    const card = await mountCard();

    clickSegment(rowByLabel(card, 'Default model'), 'Off');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'default-model',
      state: 'none',
      value: null,
    });

    clickSegment(rowByLabel(card, 'Default model'), 'On');
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'default-model',
      state: 'value',
      value: 'opusplan',
    });
  });

  it('renders the Permission mode as a Custom/Default/None combo resolving to plan', async () => {
    const card = await mountCard();
    const pm = rowByLabel(card, 'Permission mode');
    const labels = Array.from(
      pm.querySelectorAll<HTMLButtonElement>('.segmented-control-btn'),
    ).map((b) => b.textContent);
    expect(labels).toEqual(['Custom', 'Default', 'None']);
    expect(activeSegment(pm)).toBe('Default'); // absent at this scope
    expect(pm.querySelector('.config-hint')!.textContent).toContain('plan'); // resolved default
  });

  it('renders the Claude Code command first, as a Custom/Default combo with no None and no model suggestions', async () => {
    const card = await mountCard('/profiles/work');
    const command = row(card, 0);
    expect(command.querySelector('.config-row-name')!.textContent).toBe('Claude Code command');
    const labels = Array.from(
      command.querySelectorAll<HTMLButtonElement>('.segmented-control-btn'),
    ).map((b) => b.textContent);
    expect(labels).toEqual(['Custom', 'Default']);
    expect(command.querySelector('.config-hint')!.textContent).toContain('claude'); // resolved default

    clickSegment(command, 'Custom');
    const input = row(card, 0).querySelector<HTMLInputElement>('input.config-input')!;
    expect(input.placeholder).toBe('command or path, then Enter');
    expect(input.getAttribute('list')).toBeNull();
    input.value = '~/bin/claude-gateway';
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter' }));
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: '/profiles/work',
      key: 'command',
      state: 'value',
      value: '~/bin/claude-gateway',
    });
  });

  it('commits a Permission mode suggestion chip', async () => {
    const card = await mountCard();
    clickSegment(rowByLabel(card, 'Permission mode'), 'Custom'); // reveals the chips
    const pm = rowByLabel(card, 'Permission mode');
    const chipLabels = Array.from(pm.querySelectorAll<HTMLButtonElement>('.config-chip')).map(
      (c) => c.textContent,
    );
    expect(chipLabels).toEqual(['plan', 'acceptEdits', 'auto']);
    const chip = Array.from(pm.querySelectorAll<HTMLButtonElement>('.config-chip')).find(
      (c) => c.textContent === 'auto',
    )!;
    chip.click();
    await flush();
    expect(lastSetCall()).toEqual({
      profilePath: null,
      key: 'permission-mode',
      state: 'value',
      value: 'auto',
    });
  });

  it('does not focus the Custom input on the initial mount', async () => {
    // Attach before the async render so a stray focus() would actually take effect (and be caught).
    const card = createClaudeConfigCard({ profilePath: null, modelProfilePath: '/profiles/work' });
    document.body.appendChild(card);
    await flush();
    const input = row(card, 1).querySelector<HTMLInputElement>('.config-input');
    expect(input).not.toBeNull(); // opus is stored → Custom input rendered
    expect(document.activeElement).not.toBe(input);
  });

  it('focuses the input when the user switches a row to Custom', async () => {
    const card = await mountCard();
    document.body.appendChild(card);
    clickSegment(row(card, 2), 'Custom'); // sonnet, previously Default
    await flush();
    const input = row(card, 2).querySelector<HTMLInputElement>('.config-input');
    expect(document.activeElement).toBe(input);
  });

  it('does not move focus into a Custom input when a different row/control is used', async () => {
    // opus is stored -> its Custom input is visible; clicking the effort control must not grab it.
    const card = await mountCard();
    document.body.appendChild(card);
    const opusInput = row(card, 1).querySelector<HTMLInputElement>('.config-input');
    expect(opusInput).not.toBeNull();
    expect(document.activeElement).not.toBe(opusInput);

    clickSegment(row(card, 4), 'max'); // effort — unrelated to the opus row
    await flush();
    const opusInputAfter = row(card, 1).querySelector<HTMLInputElement>('.config-input');
    expect(document.activeElement).not.toBe(opusInputAfter);
  });

  it('focuses the switched row itself, not another row already in Custom', async () => {
    // sonnet is stored (Custom, later in the DOM); switching the earlier opus row must focus opus.
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_claude_settings') {
        return {
          stored: { 'sonnet-model': 'claude-opus-4-8' },
          resolved: {
            'opus-model': 'claude-opus-4-8',
            'sonnet-model': 'claude-opus-4-8',
            'haiku-model': null,
            effort: null,
          },
        };
      }
      throw new Error(`unexpected command ${cmd}`);
    });
    const card = createClaudeConfigCard({ profilePath: null, modelProfilePath: '/profiles/work' });
    document.body.appendChild(card);
    await flush();

    clickSegment(row(card, 1), 'Custom'); // opus, earlier in the DOM than the already-Custom sonnet
    await flush();
    const opusInput = row(card, 1).querySelector<HTMLInputElement>('.config-input');
    expect(document.activeElement).toBe(opusInput);
  });

  it('keeps an uncommitted Custom switch when another row is written', async () => {
    const card = await mountCard();
    clickSegment(row(card, 2), 'Custom'); // sonnet: Default -> Custom, not yet committed
    expect(activeSegment(row(card, 2))).toBe('Custom');
    expect(row(card, 2).querySelector('.config-input')).not.toBeNull();

    clickSegment(row(card, 4), 'max'); // write the unrelated effort row -> reload
    await flush();

    // The switch (and its seeded value) survives the reload triggered elsewhere.
    expect(activeSegment(row(card, 2))).toBe('Custom');
    const input = row(card, 2).querySelector<HTMLInputElement>('.config-input');
    expect(input).not.toBeNull();
    expect(input!.value).toBe('claude-opus-4-8'); // seeded from the resolved value
    const sonnetWrites = invokeMock.mock.calls.filter(
      (c) => c[0] === 'set_claude_setting' && (c[1] as { key?: string })?.key === 'sonnet-model',
    );
    expect(sonnetWrites).toHaveLength(0); // sonnet itself was never persisted
  });

  it('passes the profile path through to both commands', async () => {
    const card = await mountCard('/home/me/devora');
    expect(invokeMock).toHaveBeenCalledWith('get_claude_settings', {
      profilePath: '/home/me/devora',
    });
    clickSegment(row(card, 1), 'None');
    await flush();
    expect(lastSetCall()).toMatchObject({ profilePath: '/home/me/devora' });
  });
});

describe('ClaudeConfigCard model suggestions', () => {
  it('suggests the model IDs Claude Code offers, in the core\'s order, on every tier field and not the permission mode', async () => {
    const card = await mountCard('/profiles/work');
    expect(invokeLogOnlyMock).toHaveBeenCalledWith('get_claude_model_options', { profilePath: '/profiles/work' });
    const input = rowByLabel(card, 'Opus tier').querySelector<HTMLInputElement>('input.config-input')!;
    expect(input.placeholder).toBe('model ID, then Enter');
    const list = card.querySelector<HTMLDataListElement>(`datalist#${input.getAttribute('list')}`)!;
    expect(Array.from(list.options).map((o) => [o.value, o.label])).toEqual(MODELS.map((m) => [m.id, m.name]));
    expect(card.querySelectorAll('datalist')).toHaveLength(1);
  });

  it('leaves the fields as they are when Claude Code can\'t be asked', async () => {
    invokeLogOnlyMock.mockRejectedValue(new Error('no claude'));
    const card = await mountCard('/profiles/work');
    const input = rowByLabel(card, 'Opus tier').querySelector<HTMLInputElement>('input.config-input')!;
    expect(card.querySelector<HTMLDataListElement>(`datalist#${input.getAttribute('list')}`)!.options).toHaveLength(0);
  });

  it('asks for no models without a profile to ask for', async () => {
    createClaudeConfigCard({ profilePath: null, modelProfilePath: null });
    await flush();
    expect(invokeLogOnlyMock).not.toHaveBeenCalled();
  });
});
