import { describe, it, expect, vi, beforeEach } from 'vitest';
import { createConfigCard, ConfigFieldSpec } from '../ConfigCard';
import { invoke } from '../../../invoke';

vi.mock('../../../invoke', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

const SETTINGS = {
  stored: { 'terminal.git-shortcuts': false } as Record<string, string | boolean>,
  resolved: {
    'terminal.default-app': null,
    'terminal.git-shortcuts': false,
    'pr.auto-merge': null,
    'task-tracker.provider': null,
  } as Record<string, string | boolean | null>,
};

const FIELDS: ConfigFieldSpec[] = [
  { key: 'terminal.default-app', label: 'Default app', field: { kind: 'text', placeholder: 'nvim' } },
  { key: 'terminal.git-shortcuts', label: 'Git shortcuts', field: { kind: 'bool' } },
  { key: 'pr.auto-merge', label: 'Auto-merge', field: { kind: 'bool' }, showResolved: false, note: 'per-repo overrides apply' },
  {
    key: 'task-tracker.provider',
    label: 'Provider',
    field: {
      kind: 'enum',
      options: [
        { label: 'Inherit', state: 'default' },
        { label: 'None', state: 'value', value: '' },
        { label: 'Asana', state: 'value', value: 'asana' },
      ],
    },
  },
];

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
    if (cmd === 'get_config_settings') return SETTINGS;
    if (cmd === 'set_config_setting') {
      const state = (args as { state?: string })?.state;
      if (state !== 'value' && state !== 'default') {
        throw new Error(`unknown state: ${state}`);
      }
      return null;
    }
    throw new Error(`unexpected command ${cmd}`);
  });
});

async function mountCard(fields: ConfigFieldSpec[] = FIELDS): Promise<HTMLElement> {
  const card = createConfigCard({ title: 'Test', profilePath: null, fields });
  await flush();
  return card;
}

function row(card: HTMLElement, index: number): HTMLElement {
  return card.querySelectorAll<HTMLElement>('.config-row')[index];
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
  const calls = invokeMock.mock.calls.filter((c) => c[0] === 'set_config_setting');
  return calls.at(-1)?.[1] as Record<string, unknown> | undefined;
}

describe('createConfigCard', () => {
  it('renders one row per field', async () => {
    const card = await mountCard();
    expect(card.querySelectorAll('.config-row')).toHaveLength(FIELDS.length);
  });

  it('derives the active segment from the stored value', async () => {
    const card = await mountCard();
    // git-shortcuts stored false -> "Off"; default-app absent -> "Default".
    expect(activeSegment(row(card, 1))).toBe('Off');
    expect(activeSegment(row(card, 0))).toBe('Default');
  });

  it('writes a bool as a real "true"/"false" value string', async () => {
    const card = await mountCard();
    clickSegment(row(card, 1), 'On');
    await flush();
    expect(lastSetCall()).toMatchObject({
      key: 'terminal.git-shortcuts',
      state: 'value',
      value: 'true',
    });
  });

  it('switching a bool to Default removes the key', async () => {
    const card = await mountCard();
    clickSegment(row(card, 1), 'Default');
    await flush();
    expect(lastSetCall()).toMatchObject({ key: 'terminal.git-shortcuts', state: 'default' });
  });

  it('text field: Set reveals an input, Enter commits the value', async () => {
    const card = await mountCard();
    clickSegment(row(card, 0), 'Set');
    await flush();
    const input = row(card, 0).querySelector<HTMLInputElement>('.config-input');
    expect(input).not.toBeNull();
    input!.value = 'nvim';
    input!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await flush();
    expect(lastSetCall()).toMatchObject({
      key: 'terminal.default-app',
      state: 'value',
      value: 'nvim',
    });
  });

  it('text field: committing an empty value reverts to Default', async () => {
    const card = await mountCard();
    clickSegment(row(card, 0), 'Set');
    await flush();
    const input = row(card, 0).querySelector<HTMLInputElement>('.config-input');
    input!.value = '   ';
    input!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    await flush();
    expect(lastSetCall()).toMatchObject({ key: 'terminal.default-app', state: 'default' });
  });

  it('enum field: selecting an option stores its value', async () => {
    const card = await mountCard();
    clickSegment(row(card, 3), 'Asana');
    await flush();
    expect(lastSetCall()).toMatchObject({
      key: 'task-tracker.provider',
      state: 'value',
      value: 'asana',
    });
  });

  it('shows a resolved hint in Default mode, suppressed when showResolved is false', async () => {
    const card = await mountCard();
    // default-app (showResolved default true) shows the generic hint.
    expect(row(card, 0).querySelector('.config-hint')?.textContent).toBe('Uses Devora default');
    // pr.auto-merge has showResolved:false -> no hint, but the note is present.
    expect(row(card, 2).querySelector('.config-hint')).toBeNull();
    expect(row(card, 2).querySelector('.config-row-note')?.textContent).toBe('per-repo overrides apply');
  });

  it('hides a field whose visibleWhen returns false', async () => {
    const fields: ConfigFieldSpec[] = [
      { key: 'terminal.default-app', label: 'Default app', field: { kind: 'text' } },
      {
        key: 'task-tracker.asana.project-id',
        label: 'Project ID',
        field: { kind: 'text' },
        visibleWhen: (resolved) => resolved['task-tracker.provider'] === 'asana',
      },
    ];
    const card = await mountCard(fields);
    // provider resolves to null in SETTINGS, so the conditional field is hidden.
    expect(card.querySelectorAll('.config-row')).toHaveLength(1);
  });

  it('appends extraRows after the fields', async () => {
    const extra = document.createElement('div');
    extra.className = 'token-editor';
    const card = createConfigCard({
      title: 'Test',
      profilePath: null,
      fields: FIELDS,
      extraRows: () => [extra],
    });
    await flush();
    expect(card.querySelector('.token-editor')).not.toBeNull();
  });
});
