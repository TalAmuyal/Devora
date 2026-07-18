/**
 * Claude Models & Effort config card: edits the Opus/Sonnet/Haiku model tiers and the effort level for one scope (a profile, or `null` for user-level/global defaults).
 *
 * The three model rows are a tri-state control (`Custom | Default | None`): a concrete value, "Default" (unset here → falls through profile → user → Devora default), or "None" (impose nothing, so Claude Code uses its own default).
 * Custom models are free text (decoupled from Devora releases) with suggestion chips.
 * The Effort row is a single segmented control listing every choice at once — "Default", the supported levels (highest → lowest), and "None" — since effort is a closed set rather than free text.
 *
 * Reads via `get_claude_settings` and writes via `set_claude_setting`; after every write it re-reads so the "Default →" hint reflects the live resolution.
 * DOM: `div.claude-config-card`.
 */

import { invoke } from '../../invoke';
import { blurOnEscape } from '../focus';
import { createSegmentedControl } from './SegmentedControl';

/** Supported Claude Code effort levels, lowest → highest. Mirrors `CLAUDE_EFFORT_LEVELS` in workspace.rs. */
const EFFORT_LEVELS = ['low', 'medium', 'high', 'xhigh', 'max'] as const;

const EFFORT_LEVELS_DESC = [...EFFORT_LEVELS].reverse();

/** Suggestions only — any model id can be typed (a new model needs no Devora release). */
const MODEL_SUGGESTIONS = [
  'claude-fable-5',
  'claude-opus-4-8',
  'claude-sonnet-5',
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

  // Rows switched to Custom but not yet committed. Survives reloads (a write to another row must not collapse it); cleared when the row is left or committed.
  const pendingCustomEntry = new Set<SettingKey>();
  const customValues = new Map<SettingKey, string>();
  let settings: ClaudeSettings = { stored: {}, resolved: {} as Record<SettingKey, string | null> };
  // Serializes writes so a value-commit and a follow-on segment switch apply in order.
  let writeChain: Promise<unknown> = Promise.resolve();
  // The row whose Custom input should grab focus on the next render; null on the initial mount and after unrelated renders, so neither steals focus.
  let pendingFocusKey: SettingKey | null = null;

  const deriveMode = (key: SettingKey): Mode => {
    if (!(key in settings.stored)) return 'default';
    return settings.stored[key] === null ? 'none' : 'custom';
  };

  const displayMode = (key: SettingKey): Mode =>
    pendingCustomEntry.has(key) ? 'custom' : deriveMode(key);

  const reload = async (): Promise<void> => {
    try {
      settings = await invoke<ClaudeSettings>('get_claude_settings', {
        profilePath: options.profilePath,
      });
    } catch {
      return; // invoke already surfaced the error
    }
    for (const row of ROWS) {
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
    const rowEl = document.createElement('div');
    rowEl.className = 'claude-config-row';
    rowEl.appendChild(renderLabel(row));
    rowEl.appendChild(row.kind === 'effort' ? renderEffortControl() : renderModelControl(row));
    return rowEl;
  };

  const renderLabel = (row: RowSpec): HTMLElement => {
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
    return labelEl;
  };

  const renderControl = (segmented: HTMLElement, value: HTMLElement | null): HTMLElement => {
    const controlEl = document.createElement('div');
    controlEl.className = 'claude-config-row-control';
    controlEl.appendChild(segmented);

    const valueEl = document.createElement('div');
    valueEl.className = 'claude-config-row-value';
    if (value !== null) valueEl.appendChild(value);
    controlEl.appendChild(valueEl);

    return controlEl;
  };

  const renderModelControl = (row: RowSpec): HTMLElement => {
    const mode = displayMode(row.key);
    const segmented = createSegmentedControl<Mode>({
      items: [
        { key: 'custom', label: 'Custom' },
        { key: 'default', label: 'Default' },
        { key: 'none', label: 'None' },
      ],
      activeKey: mode,
      onSelect: (next) => onModeSelect(row, next),
    });
    const value = mode === 'custom' ? renderModelInput(row) : overrideHint(row.key, mode);
    return renderControl(segmented, value);
  };

  const renderEffortControl = (): HTMLElement => {
    const mode = displayMode('effort');
    // In "custom" mode this is the raw stored level; a hand-edited config can pin a value outside EFFORT_LEVELS, in which case no segment matches.
    const stored = customValues.get('effort');

    const segmented = createSegmentedControl<string>({
      items: [
        { key: 'default', label: 'Default' },
        ...EFFORT_LEVELS_DESC.map((level) => ({ key: level, label: level })),
        { key: 'none', label: 'None' },
      ],
      activeKey: mode === 'custom' ? (stored ?? '') : mode,
      onSelect: onEffortSelect,
    });

    let value: HTMLElement | null = null;
    if (mode !== 'custom') {
      value = overrideHint('effort', mode);
    } else if (stored && !EFFORT_LEVELS.includes(stored as (typeof EFFORT_LEVELS)[number])) {
      // Surface an off-list level (from a hand-edited config), since no segment is highlighted for it.
      value = mutedHint(`→ ${stored}`);
    }
    return renderControl(segmented, value);
  };

  const overrideHint = (key: SettingKey, mode: 'default' | 'none'): HTMLElement => {
    if (mode === 'none') {
      return mutedHint('No override — Claude Code decides');
    }
    const resolved = settings.resolved[key] ?? null;
    return mutedHint(resolved === null ? '→ Claude Code default' : `→ ${resolved}`);
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
    blurOnEscape(input);
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

    // Several Custom inputs can be visible at once; focus only the row just targeted.
    if (pendingFocusKey === row.key) {
      pendingFocusKey = null;
      queueMicrotask(() => input.focus());
    }
    return wrap;
  };

  const onModeSelect = (row: RowSpec, next: Mode): void => {
    if (next === 'custom') {
      // Local switch only — nothing is written until a concrete value is committed.
      pendingCustomEntry.add(row.key);
      if (!customValues.has(row.key)) {
        const resolved = settings.resolved[row.key];
        if (typeof resolved === 'string') customValues.set(row.key, resolved);
      }
      pendingFocusKey = row.key;
      render();
      return;
    }
    pendingCustomEntry.delete(row.key);
    persist(row.key, next);
  };

  // Effort is a closed set, so every segment commits immediately.
  const onEffortSelect = (next: string): void => {
    if (next === 'default' || next === 'none') {
      persist('effort', next);
    } else {
      customValues.set('effort', next);
      persist('effort', 'value', next);
    }
  };

  // For models, "value" means a concrete model id; a cleared field maps to "default".
  const commitModel = (row: RowSpec, raw: string): void => {
    const value = raw.trim();
    if (value === '') {
      pendingCustomEntry.delete(row.key);
      customValues.delete(row.key);
      persist(row.key, 'default');
    } else {
      customValues.set(row.key, value);
      // The row stays Custom after the write; keep focus on its input through the reload.
      pendingFocusKey = row.key;
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
