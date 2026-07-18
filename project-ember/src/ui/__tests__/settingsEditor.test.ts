import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { createSettingsEditor, SettingsEditor } from '../settingsEditor';
import { invoke } from '../../invoke';

vi.mock('../../invoke', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

interface Settings {
  stored: Record<string, string>;
  resolved: Record<string, string>;
}

const EMPTY: Settings = { stored: {}, resolved: {} };

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_x') return { stored: { a: 'stored-a' }, resolved: { a: 'stored-a' } };
    if (cmd === 'set_x') return null;
    throw new Error(`unexpected command ${cmd}`);
  });
});

afterEach(() => {
  document.body.innerHTML = '';
});

function makeEditor(
  overrides: Partial<Parameters<typeof createSettingsEditor<Settings>>[0]> = {},
): { editor: SettingsEditor<Settings>; render: ReturnType<typeof vi.fn> } {
  const render = vi.fn();
  const editor = createSettingsEditor<Settings>({
    getCommand: 'get_x',
    setCommand: 'set_x',
    profilePath: null,
    initial: EMPTY,
    render,
    ...overrides,
  });
  return { editor, render };
}

describe('createSettingsEditor', () => {
  it('reload reads via getCommand, stores the response, runs afterLoad then render', async () => {
    const order: string[] = [];
    const afterLoad = vi.fn((s: Settings) => order.push(`afterLoad:${s.stored.a}`));
    const render = vi.fn(() => order.push('render'));
    const editor = createSettingsEditor<Settings>({
      getCommand: 'get_x',
      setCommand: 'set_x',
      profilePath: '/p',
      initial: EMPTY,
      render,
      afterLoad,
    });

    await editor.reload();

    expect(invokeMock).toHaveBeenCalledWith('get_x', { profilePath: '/p' });
    expect(editor.settings.stored.a).toBe('stored-a');
    expect(order).toEqual(['afterLoad:stored-a', 'render']);
  });

  it('reload leaves state untouched and skips render when the read rejects', async () => {
    invokeMock.mockRejectedValueOnce(new Error('boom'));
    const { editor, render } = makeEditor();
    await editor.reload();
    expect(editor.settings).toBe(EMPTY);
    expect(render).not.toHaveBeenCalled();
  });

  it('persist writes via setCommand with the scope, key, state and value, then reloads', async () => {
    const { editor, render } = makeEditor();
    editor.persist('a', 'value', 'hi');
    await flush();
    expect(invokeMock).toHaveBeenCalledWith('set_x', {
      profilePath: null,
      key: 'a',
      state: 'value',
      value: 'hi',
    });
    expect(render).toHaveBeenCalled(); // reload-after-write rendered
  });

  it('persist sends a null value when none is supplied', async () => {
    const { editor } = makeEditor();
    editor.persist('a', 'default');
    await flush();
    expect(invokeMock).toHaveBeenCalledWith('set_x', {
      profilePath: null,
      key: 'a',
      state: 'default',
      value: null,
    });
  });

  it('serializes concurrent writes in call order', async () => {
    const seen: string[] = [];
    invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'set_x') seen.push((args as { value: string }).value);
      if (cmd === 'get_x') return EMPTY;
      return null;
    });
    const { editor } = makeEditor();
    editor.persist('a', 'value', 'first');
    editor.persist('a', 'value', 'second');
    await flush();
    expect(seen).toEqual(['first', 'second']);
  });

  it('a failed write does not stall later writes', async () => {
    const seen: string[] = [];
    invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
      if (cmd === 'set_x') {
        const value = (args as { value: string }).value;
        seen.push(value);
        if (value === 'first') throw new Error('write failed');
      }
      if (cmd === 'get_x') return EMPTY;
      return null;
    });
    const { editor } = makeEditor();
    editor.persist('a', 'value', 'first');
    editor.persist('a', 'value', 'second');
    await flush();
    expect(seen).toEqual(['first', 'second']);
  });

  it('beginEntry marks the field pending, seeds the draft when absent, and renders', () => {
    const { editor, render } = makeEditor();
    editor.beginEntry('a', 'seed');
    expect(editor.hasPendingEntry('a')).toBe(true);
    expect(editor.getDraft('a')).toBe('seed');
    expect(render).toHaveBeenCalledTimes(1);
  });

  it('beginEntry does not overwrite an existing draft', () => {
    const { editor } = makeEditor();
    editor.setDraft('a', 'typed');
    editor.beginEntry('a', 'seed');
    expect(editor.getDraft('a')).toBe('typed');
  });

  it('a pending entry survives a reload triggered by another write', async () => {
    const { editor } = makeEditor();
    editor.beginEntry('a', 'seed');
    editor.persist('b', 'value', 'x'); // unrelated write -> reload
    await flush();
    expect(editor.hasPendingEntry('a')).toBe(true);
    expect(editor.getDraft('a')).toBe('seed');
  });

  it('endEntry clears the pending flag but keeps the draft', () => {
    const { editor } = makeEditor();
    editor.beginEntry('a', 'seed');
    editor.endEntry('a');
    expect(editor.hasPendingEntry('a')).toBe(false);
    expect(editor.getDraft('a')).toBe('seed');
  });

  it('commit of a value stores the trimmed draft and persists it', async () => {
    const { editor } = makeEditor();
    editor.commit('a', '  hi  ');
    await flush();
    expect(editor.getDraft('a')).toBe('hi');
    expect(invokeMock).toHaveBeenCalledWith('set_x', {
      profilePath: null,
      key: 'a',
      state: 'value',
      value: 'hi',
    });
  });

  it('commit of an empty value clears the entry and draft and reverts to Default', async () => {
    const { editor } = makeEditor();
    editor.beginEntry('a', 'seed');
    editor.commit('a', '   ');
    await flush();
    expect(editor.hasPendingEntry('a')).toBe(false);
    expect(editor.getDraft('a')).toBeUndefined();
    expect(invokeMock).toHaveBeenCalledWith('set_x', {
      profilePath: null,
      key: 'a',
      state: 'default',
      value: null,
    });
  });

  it('focusOnRender focuses only the field the focus was requested for, once', async () => {
    const { editor } = makeEditor();
    const wanted = document.createElement('input');
    const other = document.createElement('input');
    document.body.append(wanted, other);

    editor.requestFocus('a');
    editor.focusOnRender('b', other); // different key -> ignored
    editor.focusOnRender('a', wanted);
    await flush();
    expect(document.activeElement).toBe(wanted);

    // The request is one-shot: a re-render of the same key does not re-grab focus.
    other.focus();
    editor.focusOnRender('a', wanted);
    await flush();
    expect(document.activeElement).toBe(other);
  });

  it('does not focus anything when no focus was requested', async () => {
    const { editor } = makeEditor();
    const input = document.createElement('input');
    document.body.append(input);
    editor.focusOnRender('a', input);
    await flush();
    expect(document.activeElement).not.toBe(input);
  });
});
