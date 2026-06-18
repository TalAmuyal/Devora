import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { CommandPalette, Command } from '../CommandPalette';

interface Setup {
  palette: CommandPalette;
  runHub: ReturnType<typeof vi.fn>;
  runShell: ReturnType<typeof vi.fn>;
}

function setup(): Setup {
  const runHub = vi.fn();
  const runShell = vi.fn();
  const commands: Command[] = [
    {
      id: 'workspace-hub',
      title: 'Workspace Hub',
      description: 'List, filter and open workspaces',
      icon: '▦',
      shortcut: ['⌃', 'S'],
      run: runHub,
    },
    {
      id: 'new-shell',
      title: 'New Shell',
      description: 'Open a fresh shell tab',
      icon: '❯',
      shortcut: ['⌃', '⇧', 'S'],
      run: runShell,
    },
  ];
  const palette = new CommandPalette({ commands });
  document.body.appendChild(palette.getElement());
  palette.load();
  return { palette, runHub, runShell };
}

function windowKey(key: string): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
}

function items(): HTMLElement[] {
  return Array.from(document.querySelectorAll('.command-palette-item')) as HTMLElement[];
}

function selectedTitle(): string | null {
  return (
    document.querySelector('.command-palette-item.selected .command-palette-item-title')
      ?.textContent ?? null
  );
}

function makeCommand(id: string, title: string, run = vi.fn()): Command {
  return { id, title, description: `${title} description`, icon: '◇', shortcut: [], run };
}

const flushPromises = async (): Promise<void> => {
  await Promise.resolve();
  await Promise.resolve();
};

describe('CommandPalette dynamic commands', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('appends resolved dynamic commands after the static ones', async () => {
    const palette = new CommandPalette({
      commands: [makeCommand('static', 'Static Command')],
      dynamicCommands: async () => [makeCommand('dynamic', 'Switch Profile: Personal')],
    });
    document.body.appendChild(palette.getElement());
    palette.load();

    expect(items()).toHaveLength(1);
    await flushPromises();

    const rows = items();
    expect(rows).toHaveLength(2);
    expect(rows[1].querySelector('.command-palette-item-title')?.textContent).toBe(
      'Switch Profile: Personal',
    );
    palette.unload();
  });

  it('re-resolves dynamic commands on every load', async () => {
    const provider = vi.fn(async () => [makeCommand('dynamic', 'Dynamic')]);
    const palette = new CommandPalette({
      commands: [makeCommand('static', 'Static Command')],
      dynamicCommands: provider,
    });
    document.body.appendChild(palette.getElement());

    palette.load();
    await flushPromises();
    palette.unload();
    palette.load();
    await flushPromises();

    expect(provider).toHaveBeenCalledTimes(2);
    palette.unload();
  });

  it('discards a resolution that lands after unload', async () => {
    let resolveProvider!: (commands: Command[]) => void;
    const palette = new CommandPalette({
      commands: [makeCommand('static', 'Static Command')],
      dynamicCommands: () => new Promise((resolve) => (resolveProvider = resolve)),
    });
    document.body.appendChild(palette.getElement());
    palette.load();
    palette.unload();

    resolveProvider([makeCommand('dynamic', 'Too Late')]);
    await flushPromises();

    expect(document.querySelectorAll('.command-palette-item')).toHaveLength(0);
  });

  it('keeps the static list when the provider rejects', async () => {
    const palette = new CommandPalette({
      commands: [makeCommand('static', 'Static Command')],
      dynamicCommands: async () => {
        throw new Error('boom');
      },
    });
    document.body.appendChild(palette.getElement());
    palette.load();
    await flushPromises();

    expect(items()).toHaveLength(1);
    palette.unload();
  });
});

describe('CommandPalette', () => {
  let s: Setup;

  beforeEach(() => {
    s = setup();
  });

  afterEach(() => {
    s.palette.unload();
    document.body.innerHTML = '';
  });

  it('renders all commands with title, description and shortcut badges', () => {
    const rows = items();
    expect(rows).toHaveLength(2);
    expect(rows[0].querySelector('.command-palette-item-title')?.textContent).toBe('Workspace Hub');
    expect(rows[0].querySelector('.command-palette-item-desc')?.textContent).toBe(
      'List, filter and open workspaces',
    );
    expect(rows[0].querySelectorAll('.command-palette-item-badge kbd')).toHaveLength(2);
    expect(rows[1].querySelector('.command-palette-item-title')?.textContent).toBe('New Shell');
    expect(rows[1].querySelectorAll('.command-palette-item-badge kbd')).toHaveLength(3);
  });

  it('selects the first command by default', () => {
    expect(selectedTitle()).toBe('Workspace Hub');
  });

  it('filters by title and description (case-insensitive)', () => {
    const input = document.querySelector('.search-input-field') as HTMLInputElement;
    input.value = 'hub';
    input.dispatchEvent(new Event('input'));
    expect(items()).toHaveLength(1);
    expect(items()[0].querySelector('.command-palette-item-title')?.textContent).toBe(
      'Workspace Hub',
    );

    input.value = 'fresh shell';
    input.dispatchEvent(new Event('input'));
    expect(items()).toHaveLength(1);
    expect(items()[0].querySelector('.command-palette-item-title')?.textContent).toBe('New Shell');
  });

  it('shows an empty state and runs nothing when no command matches', () => {
    const input = document.querySelector('.search-input-field') as HTMLInputElement;
    input.value = 'zzzz-no-match';
    input.dispatchEvent(new Event('input'));
    expect(items()).toHaveLength(0);
    expect(document.querySelector('.empty-state')).not.toBeNull();

    windowKey('Enter');
    expect(s.runHub).not.toHaveBeenCalled();
    expect(s.runShell).not.toHaveBeenCalled();
  });

  it('navigates with j/ArrowDown and k/ArrowUp, clamping at the ends', () => {
    expect(selectedTitle()).toBe('Workspace Hub');
    windowKey('j');
    expect(selectedTitle()).toBe('New Shell');
    windowKey('j'); // clamp at last
    expect(selectedTitle()).toBe('New Shell');
    windowKey('ArrowUp');
    expect(selectedTitle()).toBe('Workspace Hub');
    windowKey('k'); // clamp at first
    expect(selectedTitle()).toBe('Workspace Hub');
  });

  it('Enter executes the selected command', () => {
    windowKey('j');
    windowKey('Enter');
    expect(s.runShell).toHaveBeenCalledOnce();
    expect(s.runHub).not.toHaveBeenCalled();
  });

  it('double-click executes a command; single click only selects', () => {
    const shellRow = items()[1];
    shellRow.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    expect(selectedTitle()).toBe('New Shell');
    expect(s.runShell).not.toHaveBeenCalled();

    shellRow.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(s.runShell).toHaveBeenCalledOnce();
  });

  it('f focuses the filter input', () => {
    windowKey('f');
    const input = document.querySelector('.search-input-field');
    expect(document.activeElement).toBe(input);
  });

  it('focusSearch focuses the search field', () => {
    s.palette.focusSearch();
    const input = document.querySelector('.search-input-field');
    expect(document.activeElement).toBe(input);
  });

  it('does not navigate via j/k while the filter is focused', () => {
    const input = document.querySelector('.search-input-field') as HTMLInputElement;
    input.focus();
    expect(document.activeElement).toBe(input);
    windowKey('j');
    expect(selectedTitle()).toBe('Workspace Hub');
  });

  it('navigates and executes via ArrowDown/Enter while the filter is focused', () => {
    const input = document.querySelector('.search-input-field') as HTMLInputElement;
    input.focus();
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(selectedTitle()).toBe('New Shell');
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    expect(s.runShell).toHaveBeenCalledOnce();
  });

  it('Escape in the search field requests close', () => {
    const onRequestClose = vi.fn();
    const palette = new CommandPalette({
      commands: [makeCommand('static', 'Static Command')],
      onRequestClose,
    });
    document.body.appendChild(palette.getElement());
    palette.load();

    // Scope to this palette's own input: the shared setup() already mounted another palette.
    const input = palette.getElement().querySelector('.search-input-field') as HTMLInputElement;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
    expect(onRequestClose).toHaveBeenCalledOnce();

    palette.unload();
  });

  it('unload removes the window listener', () => {
    s.palette.unload();
    expect(() => windowKey('j')).not.toThrow();
    // After unload the DOM is cleared.
    expect(document.querySelector('.command-palette-item')).toBeNull();
  });
});
