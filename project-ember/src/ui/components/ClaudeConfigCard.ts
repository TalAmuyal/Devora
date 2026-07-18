/**
 * Claude Models & Effort config card: edits the Opus/Sonnet/Haiku model tiers and the effort level for one scope (a profile, or `null` for user-level/global defaults).
 *
 * The three model rows are a tri-state control (`Custom | Default | None`): a concrete value, "Default" (unset here → falls through profile → user → Devora default), or "None" (impose nothing, so Claude Code uses its own default).
 * Custom models are free text (decoupled from Devora releases) with suggestion chips.
 * The Effort row is a single segmented control listing every choice at once — "Default", the supported levels (highest → lowest), and "None" — since effort is a closed set rather than free text.
 *
 * The local-first read/write/re-read/focus mechanics live in the shared `settingsEditor`; this file owns only the tri-state model, the effort control, and rendering.
 * DOM: `div.settings-card` (shared chrome) containing `div.config-row`s.
 */

import { blurOnEscape } from '../focus';
import { createSettingsEditor } from '../settingsEditor';
import { createSettingsCard } from './SettingsCard';
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
  const card = createSettingsCard('Claude Models & Effort');

  const editor = createSettingsEditor<ClaudeSettings>({
    getCommand: 'get_claude_settings',
    setCommand: 'set_claude_setting',
    profilePath: options.profilePath,
    initial: { stored: {}, resolved: {} as Record<SettingKey, string | null> },
    render: () => render(),
    afterLoad: (settings, ed) => {
      for (const row of ROWS) {
        const stored = settings.stored[row.key];
        if (typeof stored === 'string') ed.setDraft(row.key, stored);
      }
    },
  });

  const deriveMode = (key: SettingKey): Mode => {
    if (!(key in editor.settings.stored)) return 'default';
    return editor.settings.stored[key] === null ? 'none' : 'custom';
  };

  const displayMode = (key: SettingKey): Mode =>
    editor.hasPendingEntry(key) ? 'custom' : deriveMode(key);

  const render = (): void => {
    // Drop everything after the header, then rebuild the rows.
    while (card.childNodes.length > 1) {
      card.removeChild(card.lastChild as ChildNode);
    }
    for (const row of ROWS) {
      card.appendChild(renderRow(row));
    }
  };

  const renderRow = (row: RowSpec): HTMLElement => {
    const rowEl = document.createElement('div');
    rowEl.className = 'config-row';
    rowEl.appendChild(renderLabel(row));
    rowEl.appendChild(row.kind === 'effort' ? renderEffortControl() : renderModelControl(row));
    return rowEl;
  };

  const renderLabel = (row: RowSpec): HTMLElement => {
    const labelEl = document.createElement('div');
    labelEl.className = 'config-row-label';
    const nameEl = document.createElement('div');
    nameEl.className = 'config-row-name';
    nameEl.textContent = row.label;
    labelEl.appendChild(nameEl);
    const hintEl = document.createElement('div');
    hintEl.className = 'config-row-hint';
    hintEl.textContent = row.hint;
    labelEl.appendChild(hintEl);
    return labelEl;
  };

  const renderControl = (segmented: HTMLElement, value: HTMLElement | null): HTMLElement => {
    const controlEl = document.createElement('div');
    controlEl.className = 'config-row-control';
    controlEl.appendChild(segmented);

    const valueEl = document.createElement('div');
    valueEl.className = 'config-row-value';
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
    const stored = editor.getDraft('effort');

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
    const resolved = editor.settings.resolved[key] ?? null;
    return mutedHint(resolved === null ? '→ Claude Code default' : `→ ${resolved}`);
  };

  const renderModelInput = (row: RowSpec): HTMLElement => {
    const wrap = document.createElement('div');
    wrap.className = 'config-combo';

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'config-input';
    input.placeholder = 'model id, then Enter';
    input.value = editor.getDraft(row.key) ?? '';
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        e.stopPropagation();
        editor.commit(row.key, input.value);
      }
    });
    blurOnEscape(input);
    wrap.appendChild(input);

    const chips = document.createElement('div');
    chips.className = 'config-chips';
    for (const suggestion of MODEL_SUGGESTIONS) {
      const chip = document.createElement('button');
      chip.className = 'config-chip';
      chip.textContent = suggestion;
      chip.addEventListener('click', () => {
        chip.blur();
        editor.commit(row.key, suggestion);
      });
      chips.appendChild(chip);
    }
    wrap.appendChild(chips);

    editor.focusOnRender(row.key, input);
    return wrap;
  };

  const onModeSelect = (row: RowSpec, next: Mode): void => {
    if (next === 'custom') {
      // Local switch only — nothing is written until a concrete value is committed. Seed the input from the resolved value.
      const resolved = editor.settings.resolved[row.key];
      editor.beginEntry(row.key, typeof resolved === 'string' ? resolved : undefined);
      return;
    }
    editor.endEntry(row.key);
    editor.persist(row.key, next);
  };

  // Effort is a closed set, so every segment commits immediately.
  const onEffortSelect = (next: string): void => {
    if (next === 'default' || next === 'none') {
      editor.persist('effort', next);
    } else {
      editor.setDraft('effort', next);
      editor.persist('effort', 'value', next);
    }
  };

  void editor.reload();
  return card;
}

function mutedHint(text: string): HTMLElement {
  const el = document.createElement('span');
  el.className = 'config-hint';
  el.textContent = text;
  return el;
}
