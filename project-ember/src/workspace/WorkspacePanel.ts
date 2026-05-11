import { invoke } from '@tauri-apps/api/core';

// Types matching the Rust command return values
interface ProfileInfo {
  name: string;
  path: string;
  repoCount: number;
}

interface WorkspaceInfo {
  id: string;
  path: string;
  taskTitle: string;
  repos: string[];
  active: boolean;
}

interface RepoStatus {
  name: string;
  branch: string;
  isDetached: boolean;
  modified: number;
  untracked: number;
}

interface RepoInfo {
  name: string;
  path: string;
}

type CategoryFilter = 'active' | 'inactive' | 'all';

export class WorkspacePanel {
  private containerEl: HTMLElement;
  private onOpenWorkspace: (path: string, title: string, repos: string[]) => void;
  private onCreateWorkspace: (path: string, title: string, repos: string[]) => void;
  private onClose: () => void;

  private profiles: ProfileInfo[] = [];
  private activeProfilePath: string | null = null;
  private workspaces: WorkspaceInfo[] = [];
  private searchFilter: string = '';
  private categoryFilter: CategoryFilter = 'active';

  private statusCache: Map<string, RepoStatus[]> = new Map();

  private showNewForm = false;
  private availableRepos: RepoInfo[] = [];
  private focusedCardIndex = -1;
  private showCheatsheet = false;

  private keyHandler = (e: KeyboardEvent) => this.handleKeyDown(e);

  constructor(
    onOpenWorkspace: (path: string, title: string, repos: string[]) => void,
    onCreateWorkspace: (path: string, title: string, repos: string[]) => void,
    onClose: () => void,
  ) {
    this.onOpenWorkspace = onOpenWorkspace;
    this.onCreateWorkspace = onCreateWorkspace;
    this.onClose = onClose;
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'ws-panel';
  }

  getElement(): HTMLElement {
    return this.containerEl;
  }

  getActiveProfilePath(): string | undefined {
    return this.activeProfilePath ?? undefined;
  }

  async load(): Promise<void> {
    window.addEventListener('keydown', this.keyHandler, true);
    try {
      this.profiles = await invoke<ProfileInfo[]>('list_profiles');
      if (this.profiles.length > 0) {
        this.activeProfilePath = this.activeProfilePath ?? this.profiles[0].path;
        await this.loadWorkspaces();
      }
    } catch (e) {
      console.error('Failed to load profiles:', e);
    }
    this.render();
    this.preloadAllStatuses();
  }

  private preloadAllStatuses(): void {
    for (const ws of this.workspaces) {
      if (this.statusCache.has(ws.id)) continue;
      invoke<RepoStatus[]>('get_workspace_status', { workspacePath: ws.path })
        .then((statuses) => {
          this.statusCache.set(ws.id, statuses);
          this.updateCardStatus(ws.id, statuses);
        })
        .catch(() => {});
    }
  }

  private updateCardStatus(wsId: string, statuses: RepoStatus[]): void {
    const cards = this.containerEl.querySelectorAll('.ws-card');
    for (const card of cards) {
      const idEl = card.querySelector('.ws-id');
      if (idEl?.textContent !== wsId) continue;

      const dot = card.querySelector('.ws-status-dot');
      if (dot) {
        const hasModifications = statuses.some((r) => r.modified > 0 || r.untracked > 0);
        dot.classList.remove('clean', 'modified');
        dot.classList.add(hasModifications ? 'modified' : 'clean');
      }

      const detail = card.querySelector('.ws-card-detail') as HTMLElement | null;
      if (detail && detail.children.length === 0) {
        detail.appendChild(this.renderRepoTable(statuses));
      }
      break;
    }
  }

  unload(): void {
    window.removeEventListener('keydown', this.keyHandler, true);
    this.searchFilter = '';
    this.categoryFilter = 'active';
    this.focusedCardIndex = -1;
    this.showNewForm = false;
    this.showCheatsheet = false;
    this.statusCache.clear();
    this.workspaces = [];
    this.profiles = [];
  }

  private isSearchFocused(): boolean {
    return this.containerEl.querySelector('.ws-search input') === document.activeElement;
  }

  private handleKeyDown(e: KeyboardEvent): void {
    if (this.isSearchFocused()) {
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopImmediatePropagation();
        (document.activeElement as HTMLElement)?.blur();
      }
      return;
    }

    if (this.showCheatsheet && e.key === 'Escape') {
      e.preventDefault();
      e.stopImmediatePropagation();
      this.showCheatsheet = false;
      this.render();
      return;
    }

    const filtered = this.filteredWorkspaces();

    switch (e.key) {
      case 'q': {
        e.preventDefault();
        e.stopImmediatePropagation();
        if (this.showCheatsheet) {
          this.showCheatsheet = false;
          this.render();
        } else {
          this.onClose();
        }
        return;
      }
      case '?': {
        e.preventDefault();
        e.stopPropagation();
        this.showCheatsheet = !this.showCheatsheet;
        this.render();
        return;
      }
      case 'f': {
        e.preventDefault();
        e.stopPropagation();
        const input = this.containerEl.querySelector<HTMLInputElement>('.ws-search input');
        input?.focus();
        return;
      }
      case '1':
        e.preventDefault();
        e.stopPropagation();
        this.categoryFilter = 'active';
        this.focusedCardIndex = -1;
        this.render();
        return;
      case '2':
        e.preventDefault();
        e.stopPropagation();
        this.categoryFilter = 'inactive';
        this.focusedCardIndex = -1;
        this.render();
        return;
      case '3':
        e.preventDefault();
        e.stopPropagation();
        this.categoryFilter = 'all';
        this.focusedCardIndex = -1;
        this.render();
        return;
      case 'j':
      case 'ArrowDown':
        e.preventDefault();
        e.stopPropagation();
        if (filtered.length > 0) {
          this.focusedCardIndex = Math.min(this.focusedCardIndex + 1, filtered.length - 1);
          this.updateCardFocus();
        }
        return;
      case 'k':
      case 'ArrowUp':
        e.preventDefault();
        e.stopPropagation();
        if (filtered.length > 0) {
          this.focusedCardIndex = Math.max(this.focusedCardIndex - 1, 0);
          this.updateCardFocus();
        }
        return;
      case 'Enter':
        e.preventDefault();
        e.stopPropagation();
        if (this.focusedCardIndex >= 0 && this.focusedCardIndex < filtered.length) {
          const ws = filtered[this.focusedCardIndex];
          this.onOpenWorkspace(ws.path, ws.taskTitle, ws.repos);
        }
        return;
    }
  }

  private updateCardFocus(): void {
    const cards = this.containerEl.querySelectorAll('.ws-card');
    cards.forEach((card, i) => {
      card.classList.toggle('ws-card-focused', i === this.focusedCardIndex);
    });
    const focusedCard = cards[this.focusedCardIndex];
    if (focusedCard) {
      focusedCard.scrollIntoView({ block: 'nearest' });
      const detail = focusedCard.querySelector('.ws-card-detail') as HTMLElement | null;
      const filtered = this.filteredWorkspaces();
      if (detail && this.focusedCardIndex < filtered.length) {
        this.loadAndRenderDetail(filtered[this.focusedCardIndex], detail);
      }
    }
  }

  private async loadWorkspaces(): Promise<void> {
    if (!this.activeProfilePath) return;
    try {
      this.workspaces = await invoke<WorkspaceInfo[]>('list_workspaces', {
        profilePath: this.activeProfilePath,
      });
    } catch (e) {
      console.error('Failed to load workspaces:', e);
      this.workspaces = [];
    }
  }

  private filteredWorkspaces(): WorkspaceInfo[] {
    let result = this.workspaces;

    if (this.categoryFilter === 'active') {
      result = result.filter((ws) => ws.active);
    } else if (this.categoryFilter === 'inactive') {
      result = result.filter((ws) => !ws.active);
    }

    if (this.searchFilter !== '') {
      const lower = this.searchFilter.toLowerCase();
      result = result.filter(
        (ws) =>
          ws.taskTitle.toLowerCase().includes(lower) ||
          ws.id.toLowerCase().includes(lower) ||
          ws.repos.some((r) => r.toLowerCase().includes(lower)),
      );
    }

    return result;
  }

  // --- Rendering ---

  private render(): void {
    this.containerEl.innerHTML = '';

    if (this.showCheatsheet) {
      this.containerEl.appendChild(this.renderCheatsheet());
      return;
    }

    this.containerEl.appendChild(this.renderHeader());
    this.containerEl.appendChild(this.renderSearch());
    this.containerEl.appendChild(this.renderCategoryTabs());
    this.containerEl.appendChild(this.renderNewButton());

    if (this.showNewForm) {
      this.containerEl.appendChild(this.renderNewForm());
    }

    if (this.profiles.length === 0) {
      this.containerEl.appendChild(this.renderEmptyMessage('No profiles configured'));
    } else {
      this.containerEl.appendChild(this.renderWorkspaceList());
    }

    this.containerEl.appendChild(this.renderLegend());
  }

  private renderHeader(): HTMLElement {
    const header = document.createElement('div');
    header.className = 'ws-header';

    const title = document.createElement('span');
    title.className = 'ws-header-title';
    title.textContent = 'Devora Ember';
    header.appendChild(title);

    if (this.profiles.length > 1) {
      header.appendChild(this.renderProfileSelector());
    }

    return header;
  }

  private renderProfileSelector(): HTMLElement {
    const select = document.createElement('select');
    select.className = 'ws-profile-selector';

    for (const profile of this.profiles) {
      const option = document.createElement('option');
      option.value = profile.path;
      option.textContent = profile.name;
      option.selected = profile.path === this.activeProfilePath;
      select.appendChild(option);
    }

    select.addEventListener('change', async () => {
      this.activeProfilePath = select.value;
      this.statusCache.clear();
      await this.loadWorkspaces();
      this.render();
    });

    return select;
  }

  private renderSearch(): HTMLElement {
    const wrapper = document.createElement('div');
    wrapper.className = 'ws-search';

    const input = document.createElement('input');
    input.type = 'text';
    input.placeholder = 'Filter workspaces...';
    input.value = this.searchFilter;

    input.addEventListener('input', () => {
      this.searchFilter = input.value;
      this.render();
      // Re-focus the input after re-render
      const newInput = this.containerEl.querySelector<HTMLInputElement>('.ws-search input');
      if (newInput) {
        newInput.focus();
        newInput.setSelectionRange(newInput.value.length, newInput.value.length);
      }
    });

    wrapper.appendChild(input);
    return wrapper;
  }

  private renderCategoryTabs(): HTMLElement {
    const bar = document.createElement('div');
    bar.className = 'ws-category-bar';

    const categories: { key: CategoryFilter; label: string; shortcut: string }[] = [
      { key: 'active', label: 'Active', shortcut: '1' },
      { key: 'inactive', label: 'Inactive', shortcut: '2' },
      { key: 'all', label: 'All', shortcut: '3' },
    ];

    for (const cat of categories) {
      const btn = document.createElement('button');
      btn.className = 'ws-category-btn' + (this.categoryFilter === cat.key ? ' ws-category-active' : '');
      btn.innerHTML = `${cat.label} <span class="ws-category-shortcut">${cat.shortcut}</span>`;
      btn.addEventListener('click', () => {
        this.categoryFilter = cat.key;
        this.render();
      });
      bar.appendChild(btn);
    }

    return bar;
  }

  private renderNewButton(): HTMLElement {
    const btn = document.createElement('button');
    btn.className = 'ws-new-btn';
    btn.textContent = '+ New Workspace';
    btn.addEventListener('click', async () => {
      this.showNewForm = !this.showNewForm;
      if (this.showNewForm && this.activeProfilePath) {
        try {
          this.availableRepos = await invoke<RepoInfo[]>('get_registered_repos', {
            profilePath: this.activeProfilePath,
          });
        } catch (e) {
          console.error('Failed to load repos:', e);
          this.availableRepos = [];
        }
      }
      this.render();
    });
    return btn;
  }

  private renderNewForm(): HTMLElement {
    const form = document.createElement('div');
    form.className = 'ws-new-form';

    // Task name input
    const nameLabel = document.createElement('label');
    nameLabel.className = 'ws-new-form-label';
    nameLabel.textContent = 'Task name';
    form.appendChild(nameLabel);

    const nameInput = document.createElement('input');
    nameInput.type = 'text';
    nameInput.className = 'ws-new-form-input';
    nameInput.placeholder = 'e.g. Fix login bug';
    form.appendChild(nameInput);

    // Repo checkboxes
    if (this.availableRepos.length > 0) {
      const repoLabel = document.createElement('label');
      repoLabel.className = 'ws-new-form-label';
      repoLabel.textContent = 'Repositories';
      form.appendChild(repoLabel);

      const repoList = document.createElement('div');
      repoList.className = 'ws-new-form-repos';

      for (const repo of this.availableRepos) {
        const item = document.createElement('label');
        item.className = 'ws-new-form-repo-item';

        const checkbox = document.createElement('input');
        checkbox.type = 'checkbox';
        checkbox.value = repo.path;

        const name = document.createElement('span');
        name.textContent = repo.name;

        item.appendChild(checkbox);
        item.appendChild(name);
        repoList.appendChild(item);
      }

      form.appendChild(repoList);
    }

    // Action buttons
    const actions = document.createElement('div');
    actions.className = 'ws-new-form-actions';

    const createBtn = document.createElement('button');
    createBtn.className = 'ws-new-form-create';
    createBtn.textContent = 'Create';
    createBtn.addEventListener('click', async () => {
      const taskName = nameInput.value.trim();
      if (!taskName || !this.activeProfilePath) return;

      const checkboxes = form.querySelectorAll<HTMLInputElement>(
        '.ws-new-form-repos input[type="checkbox"]:checked',
      );
      const repoPaths = Array.from(checkboxes).map((cb) => cb.value);

      try {
        const created = await invoke<{ path: string; name: string }>('create_workspace', {
          profilePath: this.activeProfilePath,
          repoPaths,
          taskName,
        });
        this.showNewForm = false;
        await this.loadWorkspaces();
        this.render();
        const repoNames = repoPaths.map((p) => p.split('/').pop() ?? p);
        this.onCreateWorkspace(created.path, taskName, repoNames);
      } catch (e) {
        console.error('Failed to create workspace:', e);
      }
    });

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'ws-new-form-cancel';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.addEventListener('click', () => {
      this.showNewForm = false;
      this.render();
    });

    actions.appendChild(createBtn);
    actions.appendChild(cancelBtn);
    form.appendChild(actions);

    return form;
  }

  private renderWorkspaceList(): HTMLElement {
    const list = document.createElement('div');
    list.className = 'ws-list';

    const filtered = this.filteredWorkspaces();
    if (filtered.length === 0) {
      list.appendChild(this.renderEmptyMessage('No workspaces found'));
      return list;
    }

    for (const ws of filtered) {
      list.appendChild(this.renderCard(ws));
    }
    return list;
  }

  private renderCard(ws: WorkspaceInfo): HTMLElement {
    const card = document.createElement('div');
    card.className = 'ws-card';
    card.tabIndex = 0;

    // Click to open
    card.addEventListener('click', () => {
      this.onOpenWorkspace(ws.path, ws.taskTitle, ws.repos);
    });

    // Enter key to open
    card.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        this.onOpenWorkspace(ws.path, ws.taskTitle, ws.repos);
      }
    });

    // Compact row (always visible)
    card.appendChild(this.renderCardCompact(ws));

    // Detail section (expand on hover, lazy-loaded)
    const detail = document.createElement('div');
    detail.className = 'ws-card-detail';
    card.appendChild(detail);

    // Lazy-load repo status on mouseenter
    card.addEventListener('mouseenter', () => {
      this.loadAndRenderDetail(ws, detail);
    });

    return card;
  }

  private renderCardCompact(ws: WorkspaceInfo): HTMLElement {
    const compact = document.createElement('div');
    compact.className = 'ws-card-compact';

    const dot = document.createElement('div');
    dot.className = 'ws-status-dot';
    const cached = this.statusCache.get(ws.id);
    if (cached) {
      const hasModifications = cached.some((r) => r.modified > 0 || r.untracked > 0);
      dot.classList.add(hasModifications ? 'modified' : 'clean');
    }
    compact.appendChild(dot);

    const taskName = document.createElement('div');
    taskName.className = 'ws-task-name';
    taskName.textContent = ws.taskTitle;
    compact.appendChild(taskName);

    const repoNames = document.createElement('span');
    repoNames.className = 'ws-repo-names';
    repoNames.textContent = ws.repos.join(', ');
    compact.appendChild(repoNames);

    const spacer = document.createElement('div');
    spacer.style.flex = '1';
    compact.appendChild(spacer);

    const id = document.createElement('span');
    id.className = 'ws-id';
    id.textContent = ws.id;
    compact.appendChild(id);

    const hint = document.createElement('span');
    hint.className = 'ws-expand-hint';
    hint.textContent = '▶';
    compact.appendChild(hint);

    return compact;
  }

  private loadAndRenderDetail(
    ws: WorkspaceInfo,
    detailEl: HTMLElement,
  ): void {
    if (detailEl.children.length > 0) return;

    const statuses = this.statusCache.get(ws.id);
    if (statuses) {
      detailEl.appendChild(this.renderRepoTable(statuses));
    } else {
      const loading = document.createElement('div');
      loading.className = 'ws-card-loading';
      loading.textContent = 'Loading...';
      detailEl.appendChild(loading);
    }
  }

  private renderRepoTable(statuses: RepoStatus[]): HTMLElement {
    const table = document.createElement('table');
    table.className = 'ws-repo-table';

    for (const repo of statuses) {
      const row = document.createElement('tr');

      // Repo name
      const nameCell = document.createElement('td');
      nameCell.className = 'ws-repo-table-name';
      nameCell.textContent = repo.name;
      row.appendChild(nameCell);

      // Branch
      const branchCell = document.createElement('td');
      branchCell.className = 'ws-repo-table-branch';
      if (repo.isDetached) {
        branchCell.classList.add('detached');
      }
      branchCell.textContent = repo.branch;
      row.appendChild(branchCell);

      // Status badges
      const statusCell = document.createElement('td');
      statusCell.className = 'ws-repo-table-status';

      if (repo.modified === 0 && repo.untracked === 0) {
        const badge = document.createElement('span');
        badge.className = 'ws-stat-badge clean';
        badge.textContent = '✓ clean';
        statusCell.appendChild(badge);
      } else {
        if (repo.modified > 0) {
          const badge = document.createElement('span');
          badge.className = 'ws-stat-badge modified';
          badge.textContent = `~${repo.modified} modified`;
          statusCell.appendChild(badge);
        }
        if (repo.untracked > 0) {
          const badge = document.createElement('span');
          badge.className = 'ws-stat-badge untracked';
          badge.textContent = `+${repo.untracked} untracked`;
          statusCell.appendChild(badge);
        }
      }

      row.appendChild(statusCell);
      table.appendChild(row);
    }

    return table;
  }

  private renderEmptyMessage(text: string): HTMLElement {
    const msg = document.createElement('div');
    msg.className = 'ws-empty-message';
    msg.textContent = text;
    return msg;
  }

  private renderLegend(): HTMLElement {
    const legend = document.createElement('div');
    legend.className = 'ws-legend';

    const keys: [string, string][] = [
      ['j/k', 'navigate'],
      ['Enter', 'open'],
      ['f', 'filter'],
      ['1/2/3', 'active/inactive/all'],
      ['q/Esc', 'close'],
      ['?', 'all shortcuts'],
    ];

    for (const [key, desc] of keys) {
      const item = document.createElement('span');
      item.className = 'ws-legend-item';
      item.innerHTML = `<kbd>${key}</kbd> ${desc}`;
      legend.appendChild(item);
    }

    return legend;
  }

  private renderCheatsheet(): HTMLElement {
    const sheet = document.createElement('div');
    sheet.className = 'ws-cheatsheet';

    const title = document.createElement('h2');
    title.className = 'ws-cheatsheet-title';
    title.textContent = 'Keyboard Shortcuts';
    sheet.appendChild(title);

    const sections: { heading: string; keys: [string, string][] }[] = [
      {
        heading: 'Workspace Panel',
        keys: [
          ['j / ↓', 'Move selection down'],
          ['k / ↑', 'Move selection up'],
          ['Enter', 'Open selected workspace'],
          ['f', 'Focus filter input'],
          ['Esc', 'Unfocus filter / close cheatsheet'],
          ['1', 'Show active workspaces'],
          ['2', 'Show inactive workspaces'],
          ['3', 'Show all workspaces'],
          ['q', 'Close panel'],
          ['?', 'Toggle this cheatsheet'],
        ],
      },
      {
        heading: 'Global',
        keys: [
          ['Ctrl+S', 'Toggle workspace panel'],
          ['Shift Shift', 'Toggle workspace panel (double-tap)'],
          ['Ctrl+Shift+S', 'New shell tab'],
          ['Ctrl+←/→', 'Switch tabs'],
          ['Ctrl+Shift+←/→', 'Reorder tabs'],
          ['Ctrl+Shift++', 'Increase UI size'],
          ['Ctrl+Shift+-', 'Decrease UI size'],
          ['Ctrl+=', 'Reset UI size'],
          ['Ctrl+1/2/3', 'Set UI size (small/medium/large)'],
          ['Esc', 'Dismiss overlay'],
        ],
      },
    ];

    for (const section of sections) {
      const h3 = document.createElement('h3');
      h3.className = 'ws-cheatsheet-heading';
      h3.textContent = section.heading;
      sheet.appendChild(h3);

      const table = document.createElement('table');
      table.className = 'ws-cheatsheet-table';
      for (const [key, desc] of section.keys) {
        const row = document.createElement('tr');
        const keyCell = document.createElement('td');
        keyCell.className = 'ws-cheatsheet-key';
        keyCell.innerHTML = `<kbd>${key}</kbd>`;
        const descCell = document.createElement('td');
        descCell.className = 'ws-cheatsheet-desc';
        descCell.textContent = desc;
        row.appendChild(keyCell);
        row.appendChild(descCell);
        table.appendChild(row);
      }
      sheet.appendChild(table);
    }

    const hint = document.createElement('div');
    hint.className = 'ws-cheatsheet-hint';
    hint.textContent = 'Press ? or Esc or q to go back';
    sheet.appendChild(hint);

    return sheet;
  }
}
