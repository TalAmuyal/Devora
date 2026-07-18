/**
 * Generic config card for the Settings Hub: edits dot-path config keys for one scope (a profile path, or `null` for User Defaults), reading via `get_config_settings` and writing via `set_config_setting`.
 * Used for the Terminal & Session, Pull Requests, and Task Tracker cards.
 *
 * Each field is a labeled row whose control depends on its kind:
 *  - `text`: a [Set | Default] toggle — Set reveals an input (commit with Enter or its Save button; an empty value reverts to Default).
 *  - `bool`: an [On | Off | Default] toggle.
 *  - `enum`: one segment per option (each maps to a stored value or "Default").
 * "Default" removes the key at this scope so it inherits (profile → User Defaults → Devora default); in Default mode the resolved value is shown as a hint unless `showResolved` is false.
 *
 * The local-first read/write/re-read/focus mechanics live in the shared `settingsEditor` (also used by ClaudeConfigCard); this file owns only the field kinds and rendering.
 * DOM: `div.settings-card` (shared chrome) containing `div.config-row`s.
 */

import { createSettingsEditor } from '../settingsEditor';
import { createSettingsCard } from './SettingsCard';
import { createSegmentedControl } from './SegmentedControl';
import { createCommitInput } from './CommitInput';

/** A stored config value (a string, or a bool for `bool` fields). */
export type ConfigValue = string | boolean;

interface EnumOption {
  label: string;
  /** "value" stores `value`; "default" removes the key (inherit). */
  state: 'value' | 'default';
  value?: string;
}

export type ConfigFieldKind =
  | { kind: 'text'; placeholder?: string }
  | { kind: 'bool' }
  | { kind: 'enum'; options: EnumOption[] };

export interface ConfigFieldSpec {
  /** Dot-path config key — must match a backend `CONFIG_FIELDS` entry. */
  key: string;
  label: string;
  /** Muted sub-label under the name (e.g. an env var or a usage hint). */
  hint?: string;
  field: ConfigFieldKind;
  /** Show the effective value in Default mode. Off for keys with a hidden tier (pr.auto-merge). */
  showResolved?: boolean;
  /** Always-visible caveat under the control (e.g. the auto-merge per-repo note). */
  note?: string;
  /** When set, the row is only rendered if this returns true for the current resolved values. */
  visibleWhen?: (resolved: ResolvedMap) => boolean;
}

export type ResolvedMap = Record<string, ConfigValue | null>;

interface ConfigSettings {
  /** Raw value stored at this scope per key; absent keys are omitted (→ "Default" for that scope). */
  stored: Record<string, ConfigValue>;
  /** Effective value per key after profile → user → built-in, or `null` when nothing applies. */
  resolved: ResolvedMap;
}

export interface ConfigCardOptions {
  title: string;
  /** `null` = User Defaults (global) scope; a path = that profile's scope. */
  profilePath: string | null;
  fields: ConfigFieldSpec[];
  /** Extra rows appended after the fields, rebuilt on every reload with the latest resolved values. */
  extraRows?: (resolved: ResolvedMap) => HTMLElement[];
}

type TextMode = 'set' | 'default';

export function createConfigCard(options: ConfigCardOptions): HTMLElement {
  const card = createSettingsCard(options.title);

  const editor = createSettingsEditor<ConfigSettings>({
    getCommand: 'get_config_settings',
    setCommand: 'set_config_setting',
    profilePath: options.profilePath,
    initial: { stored: {}, resolved: {} },
    render: () => render(),
  });

  const render = (): void => {
    // Drop everything after the header, then rebuild the rows.
    while (card.childNodes.length > 1) {
      card.removeChild(card.lastChild as ChildNode);
    }
    for (const field of options.fields) {
      if (field.visibleWhen && !field.visibleWhen(editor.settings.resolved)) continue;
      card.appendChild(renderRow(field));
    }
    if (options.extraRows) {
      for (const el of options.extraRows(editor.settings.resolved)) {
        card.appendChild(el);
      }
    }
  };

  const renderRow = (field: ConfigFieldSpec): HTMLElement => {
    const row = document.createElement('div');
    row.className = 'config-row';

    const labelEl = document.createElement('div');
    labelEl.className = 'config-row-label';
    const nameEl = document.createElement('div');
    nameEl.className = 'config-row-name';
    nameEl.textContent = field.label;
    labelEl.appendChild(nameEl);
    if (field.hint) {
      const hintEl = document.createElement('div');
      hintEl.className = 'config-row-hint';
      hintEl.textContent = field.hint;
      labelEl.appendChild(hintEl);
    }
    row.appendChild(labelEl);

    const control = document.createElement('div');
    control.className = 'config-row-control';
    renderControl(field, control);
    row.appendChild(control);

    return row;
  };

  const renderControl = (field: ConfigFieldSpec, control: HTMLElement): void => {
    if (field.field.kind === 'bool') {
      renderBool(field, control);
    } else if (field.field.kind === 'enum') {
      renderEnum(field, field.field.options, control);
    } else {
      renderText(field, field.field.placeholder, control);
    }
    if (field.note) {
      const note = document.createElement('div');
      note.className = 'config-row-note';
      note.textContent = field.note;
      control.appendChild(note);
    }
  };

  const renderBool = (field: ConfigFieldSpec, control: HTMLElement): void => {
    const stored = editor.settings.stored[field.key];
    const active = stored === true ? 'on' : stored === false ? 'off' : 'default';
    control.appendChild(
      createSegmentedControl({
        items: [
          { key: 'on', label: 'On' },
          { key: 'off', label: 'Off' },
          { key: 'default', label: 'Default' },
        ],
        activeKey: active,
        onSelect: (next) => {
          if (next === 'default') editor.persist(field.key, 'default');
          else editor.persist(field.key, 'value', next === 'on' ? 'true' : 'false');
        },
      }),
    );
    if (active === 'default') appendResolvedHint(field, control);
  };

  const renderEnum = (field: ConfigFieldSpec, opts: EnumOption[], control: HTMLElement): void => {
    const stored = editor.settings.stored[field.key];
    const found = opts.findIndex((o) =>
      o.state === 'default' ? stored === undefined : typeof stored === 'string' && stored === (o.value ?? ''),
    );
    const activeIndex = found >= 0 ? found : 0;
    control.appendChild(
      createSegmentedControl({
        items: opts.map((o, i) => ({ key: String(i), label: o.label })),
        activeKey: String(activeIndex),
        onSelect: (next) => {
          const opt = opts[Number(next)];
          if (opt.state === 'default') editor.persist(field.key, 'default');
          else editor.persist(field.key, 'value', opt.value ?? '');
        },
      }),
    );
    if (opts[activeIndex]?.state === 'default') appendResolvedHint(field, control);
  };

  const renderText = (field: ConfigFieldSpec, placeholder: string | undefined, control: HTMLElement): void => {
    const stored = editor.settings.stored[field.key];
    const mode: TextMode = editor.hasPendingEntry(field.key)
      ? 'set'
      : typeof stored === 'string'
        ? 'set'
        : 'default';

    control.appendChild(
      createSegmentedControl<TextMode>({
        items: [
          { key: 'set', label: 'Set' },
          { key: 'default', label: 'Default' },
        ],
        activeKey: mode,
        onSelect: (next) => {
          if (next === 'set') {
            // Local switch only — nothing is written until the value is committed. Seed the input from the stored value.
            editor.beginEntry(field.key, typeof stored === 'string' ? stored : undefined);
          } else {
            editor.endEntry(field.key);
            editor.persist(field.key, 'default');
          }
        },
      }),
    );

    const value = document.createElement('div');
    value.className = 'config-row-value';
    if (mode === 'set') {
      const savedValue = typeof stored === 'string' ? stored : '';
      const commitInput = createCommitInput({
        placeholder,
        value: editor.getDraft(field.key) ?? savedValue,
        savedValue,
        onCommit: (raw) => editor.commit(field.key, raw),
      });
      value.appendChild(commitInput.root);
      editor.focusOnRender(field.key, commitInput.input);
    } else {
      appendResolvedHint(field, value);
    }
    control.appendChild(value);
  };

  const appendResolvedHint = (field: ConfigFieldSpec, parent: HTMLElement): void => {
    if (field.showResolved === false) return;
    const hint = document.createElement('span');
    hint.className = 'config-hint';
    hint.textContent = formatResolved(editor.settings.resolved[field.key]);
    parent.appendChild(hint);
  };

  void editor.reload();
  return card;
}

function formatResolved(value: ConfigValue | null | undefined): string {
  if (value === null || value === undefined) return 'Uses Devora default';
  if (typeof value === 'boolean') return `→ ${value ? 'on' : 'off'}`;
  if (value === '') return '→ none';
  return `→ ${value}`;
}
