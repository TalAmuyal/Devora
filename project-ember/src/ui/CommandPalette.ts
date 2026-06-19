/**
 * Command Palette overlay: a searchable list of actions.
 * Class with the same lifecycle shape as WorkspaceHub (getElement/load/unload).
 * DOM: `div.command-palette`.
 */

import { createSearchInput, SearchInputHandle } from './components/SearchInput';
import { createKeyboardHintBar } from './components/KeyboardHintBar';
import { createEmptyState } from './components/EmptyState';
import { createListCursor, ListCursorHandle } from './components/listCursor';
import { isEditableElementFocused } from './focus';

export interface Command {
  id: string;
  title: string;
  description: string;
  icon: string;
  /** Equivalent keyboard shortcut, one entry per key (rendered as <kbd> badges). */
  shortcut: string[];
  run: () => void;
}

export interface CommandPaletteOptions {
  commands: Command[];
  /** Re-resolved on every load(); the result is appended after the static commands. */
  dynamicCommands?: () => Promise<Command[]>;
  /** Invoked when the palette asks to be dismissed (Escape in the focused search field). */
  onRequestClose?: () => void;
}

export class CommandPalette {
  private staticCommands: Command[];
  private dynamicCommands?: () => Promise<Command[]>;
  private onRequestClose?: () => void;
  private commands: Command[];
  private containerEl: HTMLElement;
  private searchFilter = '';
  private cursor: ListCursorHandle;
  private searchHandle: SearchInputHandle | null = null;
  private listEl: HTMLElement | null = null;
  private loadToken = 0;

  private keyHandler = (e: KeyboardEvent) => this.handleKeyDown(e);

  constructor(options: CommandPaletteOptions) {
    this.staticCommands = options.commands;
    this.dynamicCommands = options.dynamicCommands;
    this.onRequestClose = options.onRequestClose;
    this.commands = options.commands;
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'command-palette';
    this.cursor = createListCursor({
      items: () =>
        Array.from(this.listEl?.querySelectorAll<HTMLElement>('.command-palette-item') ?? []),
      activeClass: 'selected',
    });
  }

  getElement(): HTMLElement {
    return this.containerEl;
  }

  /** Move keyboard focus into the search field so the user can type a filter immediately. */
  focusSearch(): void {
    this.searchHandle?.focus();
  }

  load(): void {
    this.searchFilter = '';
    this.commands = this.staticCommands;
    window.addEventListener('keydown', this.keyHandler, true);
    this.render();
    this.cursor.set(0);

    if (this.dynamicCommands) {
      // Static commands render immediately; dynamic ones are appended when resolved.
      // The token discards a resolution from a superseded load.
      const token = ++this.loadToken;
      this.dynamicCommands()
        .then((extra) => {
          if (token !== this.loadToken) return;
          this.commands = [...this.staticCommands, ...extra];
          this.renderList();
        })
        .catch(() => {
          // Degrade quietly to the static list; the failure is already logged by the caller's invoke path.
        });
    }
  }

  unload(): void {
    window.removeEventListener('keydown', this.keyHandler, true);
    this.loadToken++;
    this.searchFilter = '';
    this.searchHandle = null;
    this.listEl = null;
    this.containerEl.innerHTML = '';
    this.cursor.set(0);
  }

  private filteredCommands(): Command[] {
    if (this.searchFilter === '') return this.commands;
    const lower = this.searchFilter.toLowerCase();
    return this.commands.filter(
      (c) =>
        c.title.toLowerCase().includes(lower) ||
        c.description.toLowerCase().includes(lower),
    );
  }

  private handleKeyDown(e: KeyboardEvent): void {
    // While the filter is focused, ArrowUp/Down and Enter route through the SearchInput callbacks, and j/k are typed literally — so ignore here.
    if (isEditableElementFocused()) {
      return;
    }

    switch (e.key) {
      case 'f':
        e.preventDefault();
        e.stopPropagation();
        this.searchHandle?.focus();
        return;
      case 'j':
      case 'ArrowDown':
        e.preventDefault();
        e.stopPropagation();
        this.cursor.move(1);
        return;
      case 'k':
      case 'ArrowUp':
        e.preventDefault();
        e.stopPropagation();
        this.cursor.move(-1);
        return;
      case 'Enter':
        e.preventDefault();
        e.stopPropagation();
        this.execute();
        return;
    }
  }

  private execute(): void {
    this.filteredCommands()[this.cursor.index()]?.run();
  }

  private render(): void {
    this.searchHandle = null;
    this.listEl = null;
    this.containerEl.innerHTML = '';

    const card = document.createElement('div');
    card.className = 'command-palette-card';

    const titlebar = document.createElement('div');
    titlebar.className = 'command-palette-titlebar';
    const dot = document.createElement('span');
    dot.className = 'command-palette-titlebar-dot';
    titlebar.appendChild(dot);
    const titleText = document.createElement('span');
    titleText.className = 'command-palette-titlebar-text';
    titleText.textContent = 'Command Palette';
    titlebar.appendChild(titleText);
    card.appendChild(titlebar);

    const head = document.createElement('div');
    head.className = 'command-palette-head';
    this.searchHandle = createSearchInput({
      placeholder: 'Type a command…',
      value: this.searchFilter,
      icon: '⌕',
      onInput: (value) => {
        this.searchFilter = value;
        this.renderList();
        this.cursor.set(0);
      },
      onEscape: () => this.onRequestClose?.(),
      onArrowDown: () => this.cursor.move(1),
      onArrowUp: () => this.cursor.move(-1),
      onEnter: () => this.execute(),
    });
    head.appendChild(this.searchHandle.element);
    card.appendChild(head);

    const list = document.createElement('div');
    list.className = 'command-palette-list';
    this.listEl = list;
    card.appendChild(list);
    this.renderList();

    const footer = document.createElement('div');
    footer.className = 'command-palette-footer';
    footer.appendChild(
      createKeyboardHintBar({
        hints: [
          { keys: '↑/↓', description: 'navigate' },
          { keys: 'Enter', description: 'run' },
          { keys: 'Esc', description: 'close' },
        ],
      }),
    );
    card.appendChild(footer);

    this.containerEl.appendChild(card);
  }

  private renderList(): void {
    if (!this.listEl) return;
    this.listEl.innerHTML = '';

    const filtered = this.filteredCommands();
    if (filtered.length === 0) {
      this.listEl.appendChild(createEmptyState('No matching commands'));
      this.cursor.sync();
      return;
    }

    for (let i = 0; i < filtered.length; i++) {
      this.listEl.appendChild(this.renderItem(filtered[i], i));
    }
    this.cursor.sync();
  }

  private renderItem(command: Command, index: number): HTMLElement {
    const item = document.createElement('div');
    item.className = 'command-palette-item';

    const icon = document.createElement('span');
    icon.className = 'command-palette-item-icon';
    icon.textContent = command.icon;
    item.appendChild(icon);

    const body = document.createElement('div');
    body.className = 'command-palette-item-body';

    const title = document.createElement('div');
    title.className = 'command-palette-item-title';
    title.textContent = command.title;
    body.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'command-palette-item-desc';
    desc.textContent = command.description;
    body.appendChild(desc);

    item.appendChild(body);

    const badge = document.createElement('span');
    badge.className = 'command-palette-item-badge';
    for (const key of command.shortcut) {
      const kbd = document.createElement('kbd');
      kbd.textContent = key;
      badge.appendChild(kbd);
    }
    item.appendChild(badge);

    item.addEventListener('click', () => {
      this.cursor.set(index);
    });
    item.addEventListener('dblclick', () => {
      this.cursor.set(index);
      command.run();
    });

    return item;
  }
}
