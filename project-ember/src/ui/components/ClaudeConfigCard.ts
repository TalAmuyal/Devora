/**
 * Claude Models & Effort config card: edits the Opus/Sonnet/Haiku model tiers and the effort level for one scope (a profile, or `null` for user-level/global defaults).
 *
 * Each row is a tri-state control (`Custom | Default | None`): a concrete value, "Default" (unset here → falls through profile → user → Devora default), or "None" (impose nothing, so Claude Code uses its own default).
 * Custom models are free text (decoupled from Devora releases) with suggestion chips; effort is a dropdown of the supported levels.
 *
 * Reads via `get_claude_settings` and writes via `set_claude_setting`; after every write it re-reads so the "Default →" hint reflects the live resolution.
 * DOM: `div.claude-config-card`.
 */

import { invoke } from '../../invoke';
import { createSegmentedControl } from './SegmentedControl';
import { createDropdownMenu } from './DropdownMenu';

/** Supported Claude Code effort levels. Mirrors `CLAUDE_EFFORT_LEVELS` in workspace.rs. */
const EFFORT_LEVELS = ['low', 'medium', 'high', 'xhigh', 'max'] as const;

/** Suggestions only — any model id can be typed (a new model needs no Devora release). */
const MODEL_SUGGESTIONS = [
  'claude-fable-5',
  'claude-opus-4-8',
  'claude-opus-4-7',
  'claude-sonnet-4-6',
];

type SettingKey = 'opus-model' | 'sonnet-model' | 'haiku-model' | 'effort';
type Mode = 'custom' | 'default' | 'none';

interface RowSpec {
  key: SettingKey;
  label: string;
  hint: string; // the env var / flag it drives, shown muted
  kind: 'model' | 'effort';
}

const ROWS: RowSpec[] = [
  { key: 'opus-model', label: 'Opus tier', hint: 'ANTHROPIC_DEFAULT_OPUS_MODEL', kind: 'model' },
  { key: 'sonnet-model', label: 'Sonnet tier', hint: 'ANTHROPIC_DEFAULT_SONNET_MODEL', kind: 'model' },
  { key: 'haiku-model', label: 'Haiku tier', hint: 'ANTHROPIC_DEFAULT_HAIKU_MODEL', kind: 'model' },
  { key: 'effort', label: 'Effort', hint: '--effort', kind: 'effort' },
];

interface ClaudeSettings {
  /** Raw value stored at this scope: a string, `null` (None), or the key is absent. */
  stored: Partial<Record<SettingKey, string | null>>;
  /** Effective value after full resolution: a string, or `null` meaning None. */
  resolved: Record<SettingKey, string | null>;
}

export interface ClaudeConfigCardOptions {
  /** `null` = user-level/global scope; a path = that profile's scope. */
  profilePath: string | null;
}

export function createClaudeConfigCard(options: ClaudeConfigCardOptions): HTMLElement {
  const card = document.createElement('div');
  card.className = 'claude-config-card';

  const modes = new Map<SettingKey, Mode>();
  const customValues = new Map<SettingKey, string>();
  let settings: ClaudeSettings = { stored: {}, resolved: {} as Record<SettingKey, string | null> };
  // Serializes writes so a value-commit and a follow-on segment switch apply in order.
  let writeChain: Promise<unknown> = Promise.resolve();

  const deriveMode = (key: SettingKey): Mode => {
    if (!(key in settings.stored)) return 'default';
    return settings.stored[key] === null ? 'none' : 'custom';
  };

  const reload = async (): Promise<void> => {
    try {
      settings = await invoke<ClaudeSettings>('get_claude_settings', {
        profilePath: options.profilePath,
      });
    } catch {
      return; // invoke already surfaced the error
    }
    for (const row of ROWS) {
      modes.set(row.key, deriveMode(row.key));
      const stored = settings.stored[row.key];
      if (typeof stored === 'string') customValues.set(row.key, stored);
    }
    render();
  };

  // `state` is the backend vocabulary: "value" writes a string, "none" writes null, "default" removes the key.
  // (The UI "custom" mode maps to the "value" write.)
  const persist = (key: SettingKey, state: 'value' | 'none' | 'default', value?: string): void => {
    writeChain = writeChain
      .then(() =>
        invoke('set_claude_setting', {
          profilePath: options.profilePath,
          key,
          state,
          value: value ?? null,
        }),
      )
      .then(
        () => reload(),
        () => {}, // invoke already surfaced the error
      );
  };

  const render = (): void => {
    card.innerHTML = '';

    const header = document.createElement('div');
    header.className = 'claude-config-card-header';
    header.textContent = 'Claude Models & Effort';
    card.appendChild(header);

    for (const row of ROWS) {
      card.appendChild(renderRow(row));
    }
  };

  const renderRow = (row: RowSpec): HTMLElement => {
    const mode = modes.get(row.key) ?? 'default';

    const rowEl = document.createElement('div');
    rowEl.className = 'claude-config-row';

    const labelEl = document.createElement('div');
    labelEl.className = 'claude-config-row-label';
    const nameEl = document.createElement('div');
    nameEl.className = 'claude-config-row-name';
    nameEl.textContent = row.label;
    labelEl.appendChild(nameEl);
    const hintEl = document.createElement('div');
    hintEl.className = 'claude-config-row-env';
    hintEl.textContent = row.hint;
    labelEl.appendChild(hintEl);
    rowEl.appendChild(labelEl);

    const controlEl = document.createElement('div');
    controlEl.className = 'claude-config-row-control';

    controlEl.appendChild(
      createSegmentedControl<Mode>({
        items: [
          { key: 'custom', label: 'Custom' },
          { key: 'default', label: 'Default' },
          { key: 'none', label: 'None' },
        ],
        activeKey: mode,
        onSelect: (next) => onModeSelect(row, next),
      }),
    );

    const valueEl = document.createElement('div');
    valueEl.className = 'claude-config-row-value';
    valueEl.appendChild(renderValue(row, mode));
    controlEl.appendChild(valueEl);

    rowEl.appendChild(controlEl);
    return rowEl;
  };

  const renderValue = (row: RowSpec, mode: Mode): HTMLElement => {
    if (mode === 'none') {
      return mutedHint('No override — Claude Code decides');
    }
    if (mode === 'default') {
      const resolved = settings.resolved[row.key] ?? null;
      return mutedHint(resolved === null ? '→ Claude Code default' : `→ ${resolved}`);
    }
    // mode === 'custom'
    return row.kind === 'effort' ? renderEffortPicker(row) : renderModelInput(row);
  };

  const renderModelInput = (row: RowSpec): HTMLElement => {
    const wrap = document.createElement('div');
    wrap.className = 'claude-config-combo';

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'claude-config-input';
    input.placeholder = 'model id, then Enter';
    input.value = customValues.get(row.key) ?? '';
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        e.stopPropagation();
        commitModel(row, input.value);
      }
    });
    wrap.appendChild(input);

    const chips = document.createElement('div');
    chips.className = 'claude-config-chips';
    for (const suggestion of MODEL_SUGGESTIONS) {
      const chip = document.createElement('button');
      chip.className = 'claude-config-chip';
      chip.textContent = suggestion;
      chip.addEventListener('click', () => {
        chip.blur();
        commitModel(row, suggestion);
      });
      chips.appendChild(chip);
    }
    wrap.appendChild(chips);

    // Focus the field when the user just switched into Custom mode.
    queueMicrotask(() => input.focus());
    return wrap;
  };

  const renderEffortPicker = (row: RowSpec): HTMLElement => {
    const current = customValues.get(row.key) ?? settings.resolved[row.key] ?? 'xhigh';
    return createDropdownMenu({
      triggerLabel: EFFORT_LEVELS.includes(current as (typeof EFFORT_LEVELS)[number])
        ? current
        : 'Select…',
      items: EFFORT_LEVELS.map((level) => ({
        kind: 'option' as const,
        label: level,
        checked: level === current,
        onSelect: () => {
          customValues.set(row.key, level);
          persist(row.key, 'value', level);
        },
      })),
    }).element;
  };

  const onModeSelect = (row: RowSpec, next: Mode): void => {
    if (next === 'custom') {
      // Local switch only — nothing is written until a concrete value is committed.
      modes.set(row.key, 'custom');
      if (!customValues.has(row.key)) {
        const resolved = settings.resolved[row.key];
        if (typeof resolved === 'string') customValues.set(row.key, resolved);
      }
      render();
      return;
    }
    persist(row.key, next);
  };

  // For models, "value" means a concrete model id; a cleared field maps to "default".
  const commitModel = (row: RowSpec, raw: string): void => {
    const value = raw.trim();
    if (value === '') {
      customValues.delete(row.key);
      persist(row.key, 'default');
    } else {
      customValues.set(row.key, value);
      persist(row.key, 'value', value);
    }
  };

  void reload();
  return card;
}

function mutedHint(text: string): HTMLElement {
  const el = document.createElement('span');
  el.className = 'claude-config-hint';
  el.textContent = text;
  return el;
}
