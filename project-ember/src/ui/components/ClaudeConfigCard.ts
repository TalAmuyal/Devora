/**
 * Claude launch settings card: edits the default model, the Opus/Sonnet/Haiku model tiers, the effort level, and the permission mode for one scope (a profile, or `null` for user-level/global defaults).
 *
 * Most rows are a tri-state "combo" control (`Custom | Default | None`): a concrete value, "Default" (unset here → falls through profile → user → Devora default), or "None" (impose nothing, so Claude Code uses its own default). Custom values are free text (decoupled from Devora releases): the permission mode's Custom has suggestion chips, and a model tier's Custom suggests the full model IDs Claude Code offers (`get_claude_model_options`, in the core's order) as its input's own list; the model tiers have no Devora default, so their Default leaves the model to Claude Code.
 * The Effort row is a single segmented control listing every choice at once — "Default", the supported levels (highest → lowest), and "None" — since effort is a closed set rather than free text.
 * The Default model row is a 2-state On/Off toggle: On imposes `opusplan` (via ANTHROPIC_MODEL); Off omits the var so Claude Code picks its own model.
 *
 * The local-first read/write/re-read/focus mechanics live in the shared `settingsEditor`; this file owns only the per-row control shapes and rendering.
 * DOM: `div.settings-card` (shared chrome) containing `div.config-row`s.
 */

import { createSettingsEditor } from '../settingsEditor';
import { createSettingsCard } from './SettingsCard';
import { createSegmentedControl } from './SegmentedControl';
import { createCommitInput } from './CommitInput';
import { invokeLogOnly } from '../../invoke';

/** The model alias imposed while the Default model toggle is On. Mirrors the `default-model` default in workspace.rs. */
const DEFAULT_MODEL_ALIAS = 'opusplan';

type SettingKey =
  | 'command'
  | 'opus-model'
  | 'sonnet-model'
  | 'haiku-model'
  | 'effort'
  | 'default-model'
  | 'permission-mode';
type Mode = 'custom' | 'default' | 'none';

interface RowSpec {
  key: SettingKey;
  label: string;
  hint: string; // the env var / flag it drives, shown muted
  kind: 'combo' | 'effort' | 'toggle';
  placeholder?: string; // combo only: the free-text input placeholder
  noNone?: boolean; // combo only: the setting always needs a value (the command), so it has no None
}

const ROWS: RowSpec[] = [
  { key: 'command', label: 'Claude Code command', hint: 'a command name or a path', kind: 'combo', placeholder: 'command or path, then Enter', noNone: true },
  { key: 'opus-model', label: 'Opus tier', hint: 'ANTHROPIC_DEFAULT_OPUS_MODEL', kind: 'combo', placeholder: 'model ID, then Enter' },
  { key: 'sonnet-model', label: 'Sonnet tier', hint: 'ANTHROPIC_DEFAULT_SONNET_MODEL', kind: 'combo', placeholder: 'model ID, then Enter' },
  { key: 'haiku-model', label: 'Haiku tier', hint: 'ANTHROPIC_DEFAULT_HAIKU_MODEL', kind: 'combo', placeholder: 'model ID, then Enter' },
  { key: 'effort', label: 'Effort', hint: '--effort', kind: 'effort' },
  { key: 'default-model', label: 'Default model', hint: 'ANTHROPIC_MODEL', kind: 'toggle' },
  { key: 'permission-mode', label: 'Permission mode', hint: '--permission-mode', kind: 'combo', placeholder: 'permission mode, then Enter' },
];

interface ClaudeSettings {
  choices: { effort_levels: string[]; permission_mode_suggestions: string[] };
  /** Raw value stored at this scope: a string, `null` (None), or the key is absent. */
  stored: Partial<Record<SettingKey, string | null>>;
  /** Effective value after full resolution: a string, or `null` meaning None. */
  resolved: Record<SettingKey, string | null>;
}

export interface ClaudeConfigCardOptions {
  /** `null` = user-level/global scope; a path = that profile's scope. */
  profilePath: string | null;
  /** The profile whose agents' models the tier fields suggest: this scope's, or the active one for the global scope; `null` suggests none. */
  modelProfilePath: string | null;
}

/** A full model ID Claude Code offers, with its model's name (`get_claude_model_options`). */
interface TierModel {
  id: string;
  name: string;
}

let modelListCount = 0;

export function createClaudeConfigCard(options: ClaudeConfigCardOptions): HTMLElement {
  const card = createSettingsCard('Claude Launch Settings');

  // One list for the three tier inputs, filled when the core answers: they work at once, and a failed lookup (no CLI,
  // not logged in) leaves it empty, with a log line and no banner.
  const modelList = document.createElement('datalist');
  modelList.id = `claude-model-ids-${++modelListCount}`;
  const suggestModels = (models: TierModel[]): void => {
    modelList.replaceChildren(...models.map((model) => {
      const option = document.createElement('option');
      option.value = model.id;
      option.label = model.name;
      return option;
    }));
  };
  // Asked again after every load: a changed tier changes what Claude Code's aliases resolve to.
  const refreshModels = (): void => {
    if (options.modelProfilePath === null) return;
    invokeLogOnly<TierModel[]>('get_claude_model_options', { profilePath: options.modelProfilePath })
      .then(suggestModels)
      .catch(() => {});
  };

  const editor = createSettingsEditor<ClaudeSettings>({
    getCommand: 'get_claude_settings',
    setCommand: 'set_claude_setting',
    profilePath: options.profilePath,
    initial: { stored: {}, resolved: {} as Record<SettingKey, string | null>, choices: { effort_levels: [], permission_mode_suggestions: [] } },
    render: () => render(),
    afterLoad: (settings, ed) => {
      refreshModels();
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
    card.appendChild(modelList);
  };

  const renderRow = (row: RowSpec): HTMLElement => {
    const rowEl = document.createElement('div');
    rowEl.className = 'config-row';
    rowEl.appendChild(renderLabel(row));
    rowEl.appendChild(renderControlFor(row));
    return rowEl;
  };

  const renderControlFor = (row: RowSpec): HTMLElement => {
    if (row.kind === 'effort') return renderEffortControl();
    if (row.kind === 'toggle') return renderToggleControl(row);
    return renderComboControl(row);
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

  const renderComboControl = (row: RowSpec): HTMLElement => {
    const mode = displayMode(row.key);
    const segmented = createSegmentedControl<Mode>({
      items: [
        { key: 'custom' as Mode, label: 'Custom' },
        { key: 'default' as Mode, label: 'Default' },
        ...(row.noNone ? [] : [{ key: 'none' as Mode, label: 'None' }]),
      ],
      activeKey: mode,
      onSelect: (next) => onModeSelect(row, next),
    });
    const value = mode === 'custom' ? renderComboInput(row) : overrideHint(row.key, mode);
    return renderControl(segmented, value);
  };

  // The Default model toggle projects the tri-state config onto two states: On ⇒ impose the alias (env var set), Off ⇒ None (env var omitted).
  // On writes the alias explicitly (not "Default") so a profile On overrides a user-scope Off; the displayed state follows the resolved value so an inheriting scope reads correctly.
  const renderToggleControl = (row: RowSpec): HTMLElement => {
    const on = (editor.settings.resolved[row.key] ?? null) !== null;
    const segmented = createSegmentedControl<'on' | 'off'>({
      items: [
        { key: 'on', label: 'On' },
        { key: 'off', label: 'Off' },
      ],
      activeKey: on ? 'on' : 'off',
      onSelect: (next) => {
        if (next === 'on') editor.persist(row.key, 'value', DEFAULT_MODEL_ALIAS);
        else editor.persist(row.key, 'none');
      },
    });
    return renderControl(segmented, mutedHint(on ? `→ ${DEFAULT_MODEL_ALIAS}` : '→ not set'));
  };

  const renderEffortControl = (): HTMLElement => {
    const mode = displayMode('effort');
    // A hand-edited off-list effort has no matching segment but is still shown as a hint.
    const levels = editor.settings.choices?.effort_levels ?? [];
    const stored = editor.getDraft('effort');

    const segmented = createSegmentedControl<string>({
      items: [
        { key: 'default', label: 'Default' },
        ...[...levels].reverse().map((level) => ({ key: level, label: level })),
        { key: 'none', label: 'None' },
      ],
      activeKey: mode === 'custom' ? (stored ?? '') : mode,
      onSelect: onEffortSelect,
    });

    let value: HTMLElement | null = null;
    if (mode !== 'custom') {
      value = overrideHint('effort', mode);
    } else if (stored && !levels.includes(stored)) {
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

  const renderComboInput = (row: RowSpec): HTMLElement => {
    const wrap = document.createElement('div');
    wrap.className = 'config-combo';

    const stored = editor.settings.stored[row.key];
    const savedValue = typeof stored === 'string' ? stored : '';
    const commitInput = createCommitInput({
      placeholder: row.placeholder ?? '',
      value: editor.getDraft(row.key) ?? '',
      savedValue,
      whitespace: 'trim',
      onCommit: (value) => editor.commit(row.key, value),
    });
    if (row.key.endsWith('-model')) commitInput.input.setAttribute('list', modelList.id);
    wrap.appendChild(commitInput.root);

    const chips = document.createElement('div');
    chips.className = 'config-chips';
    const choices = editor.settings.choices;
    // The permission mode's suggestions are chips; a model tier's are its input's list (`modelList`), since a subscription offers too many models for chips.
    const suggestions = row.key === 'permission-mode' ? choices?.permission_mode_suggestions : [];
    for (const suggestion of suggestions ?? []) {
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

    editor.focusOnRender(row.key, commitInput.input);
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
