/**
 * The local-first editing engine shared by the Settings Hub's config cards (ClaudeConfigCard, ConfigCard):
 * read a scope's settings, write one key at a time with no Save button, and re-read after every write so hints reflect the live resolution.
 *
 * It owns the mechanics that both cards must get identically right (and have historically had to fix in lockstep):
 *  - serialized writes, so a value-commit and a follow-on segment switch apply in order;
 *  - reveal-input drafts whose "switched but not yet committed" state survives a reload triggered by another field;
 *  - single-shot, per-field focus, so revealing one input never steals focus into another.
 *
 * It deliberately knows nothing about a card's value model (tri-state vs Set/Default), field kinds, or rendering — those stay in each card.
 */

import { invoke } from '../invoke';

export interface SettingsEditor<S> {
  /** The latest settings read from the backend. */
  readonly settings: S;
  /** Re-read the scope's settings, run `afterLoad`, then re-render. A read failure is surfaced by `invoke` and leaves state untouched. */
  reload(): Promise<void>;
  /** Write one key (`state`/`value` is the backend vocabulary), then reload. Serialized behind earlier writes. */
  persist(key: string, state: string, value?: string): void;

  /** Whether `key`'s reveal-input has been switched on but not yet committed. */
  hasPendingEntry(key: string): boolean;
  /** Reveal `key`'s input (seeding the draft from `seed` when it has none), focus it, and re-render. Writes nothing. */
  beginEntry(key: string, seed?: string): void;
  /** Collapse `key`'s reveal-input (left without committing); the draft is kept for a later re-entry. */
  endEntry(key: string): void;

  getDraft(key: string): string | undefined;
  setDraft(key: string, value: string): void;
  /** Commit `key`'s reveal-input: a blank value reverts to Default (clearing the draft); otherwise the trimmed value is stored, keeping focus through the reload. */
  commit(key: string, raw: string): void;

  /** Grab focus for `key`'s input on the next render. */
  requestFocus(key: string): void;
  /** Focus `input` iff the focus was requested for `key`; the request is one-shot. */
  focusOnRender(key: string, input: HTMLInputElement): void;
}

export interface SettingsEditorConfig<S> {
  /** Command reading `{ stored, resolved }` for the scope. */
  getCommand: string;
  /** Command writing one key at the scope. */
  setCommand: string;
  /** `null` = user-level/global scope; a path = that profile's scope. */
  profilePath: string | null;
  /** State to expose before the first load completes. */
  initial: S;
  render: () => void;
  /** Runs after each successful read, before render — e.g. to seed drafts from the stored values. */
  afterLoad?: (settings: S, editor: SettingsEditor<S>) => void;
}

export function createSettingsEditor<S>(config: SettingsEditorConfig<S>): SettingsEditor<S> {
  let settings = config.initial;
  // Serializes writes so a value-commit and a follow-on segment switch apply in order.
  let writeChain: Promise<unknown> = Promise.resolve();
  // Keys switched to a reveal-input but not yet committed. Survives reloads (a write to another key must not collapse it); cleared when the key is left or committed.
  const pendingEntry = new Set<string>();
  const drafts = new Map<string, string>();
  // The key whose input should grab focus on the next render; null on the initial mount and after unrelated renders, so neither steals focus.
  let pendingFocusKey: string | null = null;

  const reload = async (): Promise<void> => {
    try {
      settings = await invoke<S>(config.getCommand, { profilePath: config.profilePath });
    } catch {
      return; // invoke already surfaced the error
    }
    config.afterLoad?.(settings, editor);
    config.render();
  };

  const persist = (key: string, state: string, value?: string): void => {
    writeChain = writeChain
      .then(() =>
        invoke(config.setCommand, {
          profilePath: config.profilePath,
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

  const editor: SettingsEditor<S> = {
    get settings() {
      return settings;
    },
    reload,
    persist,

    hasPendingEntry: (key) => pendingEntry.has(key),
    beginEntry: (key, seed) => {
      pendingEntry.add(key);
      if (seed !== undefined && !drafts.has(key)) drafts.set(key, seed);
      pendingFocusKey = key;
      config.render();
    },
    endEntry: (key) => {
      pendingEntry.delete(key);
    },

    getDraft: (key) => drafts.get(key),
    setDraft: (key, value) => {
      drafts.set(key, value);
    },
    commit: (key, raw) => {
      const value = raw.trim();
      if (value === '') {
        pendingEntry.delete(key);
        drafts.delete(key);
        persist(key, 'default');
      } else {
        drafts.set(key, value);
        // The field stays revealed after the write; keep focus on its input through the reload.
        pendingFocusKey = key;
        persist(key, 'value', value);
      }
    },

    requestFocus: (key) => {
      pendingFocusKey = key;
    },
    focusOnRender: (key, input) => {
      if (pendingFocusKey === key) {
        pendingFocusKey = null;
        queueMicrotask(() => input.focus());
      }
    },
  };

  return editor;
}
