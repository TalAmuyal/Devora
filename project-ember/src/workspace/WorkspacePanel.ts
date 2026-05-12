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
  private statusErrors: Map<string, string> = new Map();

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
          this.statusErrors.delete(ws.id);
          this.updateCardStatus(ws.id, statuses);
        })
        .catch((err) => {
          const errorMsg = err instanceof Error ? err.message : String(err);
          this.statusErrors.set(ws.id, errorMsg);
          this.updateMasterItemError(ws.id);
        });
    }
  }

  private updateMasterItemError(wsId: string): void {
    const items = this.containerEl.querySelectorAll('.ws-master-item');
    for (const item of items) {
      if ((item as HTMLElement).dataset.wsIndex === undefined) continue;
      const idEl = item.querySelector('.ws-id');
      if (idEl?.textContent !== wsId) continue;

      item.classList.add('ws-invalid');
      const dot = item.querySelector('.ws-status-dot');
      if (dot) {
        dot.classList.remove('clean', 'modified', 'pending', 'error');
        dot.classList.add('error');
      }

      const focused = this.filteredWorkspaces()[this.focusedCardIndex];
      if (focused && focused.id === wsId) {
        this.updateDetailPanel();
      }
      break;
    }
  }

  private updateCardStatus(wsId: string, statuses: RepoStatus[]): void {
    const items = this.containerEl.querySelectorAll('.ws-master-item');
    for (const item of items) {
      const idEl = item.querySelector('.ws-id');
      if (idEl?.textContent !== wsId) continue;

      item.classList.remove('ws-invalid');
      const dot = item.querySelector('.ws-status-dot');
      if (dot) {
        const hasModifications = statuses.some((r) => r.modified > 0 || r.untracked > 0);
        dot.classList.remove('clean', 'modified', 'pending', 'error');
        dot.classList.add(hasModifications ? 'modified' : 'clean');
      }

      const focused = this.filteredWorkspaces()[this.focusedCardIndex];
      if (focused && focused.id === wsId) {
        this.updateDetailPanel();
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
    this.statusErrors.clear();
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
    const items = this.containerEl.querySelectorAll('.ws-master-item');
    items.forEach((item, i) => {
      item.classList.toggle('ws-master-focused', i === this.focusedCardIndex);
    });
    const focusedItem = items[this.focusedCardIndex];
    if (focusedItem) {
      focusedItem.scrollIntoView({ block: 'nearest' });
    }
    this.updateDetailPanel();
  }

  private updateDetailPanel(): void {
    const filtered = this.filteredWorkspaces();
    const detailPanel = this.containerEl.querySelector('.ws-detail-panel');
    if (!detailPanel) return;

    detailPanel.innerHTML = '';

    if (this.focusedCardIndex >= 0 && this.focusedCardIndex < filtered.length) {
      const ws = filtered[this.focusedCardIndex];
      detailPanel.appendChild(this.renderDetailPanel(ws));
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
      const filtered = this.filteredWorkspaces();

      if (filtered.length > 0 && this.focusedCardIndex < 0) {
        this.focusedCardIndex = 0;
      }

      const split = document.createElement('div');
      split.className = 'ws-split';

      // Master panel (left)
      const masterPanel = document.createElement('div');
      masterPanel.className = 'ws-master-panel';

      if (filtered.length === 0) {
        masterPanel.appendChild(this.renderEmptyMessage('No workspaces found'));
      } else {
        for (let i = 0; i < filtered.length; i++) {
          masterPanel.appendChild(this.renderMasterItem(filtered[i], i));
        }
      }

      split.appendChild(masterPanel);

      // Detail panel (right)
      const detailPanel = document.createElement('div');
      detailPanel.className = 'ws-detail-panel';

      if (this.focusedCardIndex >= 0 && this.focusedCardIndex < filtered.length) {
        detailPanel.appendChild(this.renderDetailPanel(filtered[this.focusedCardIndex]));
      }

      split.appendChild(detailPanel);
      this.containerEl.appendChild(split);
    }

    this.containerEl.appendChild(this.renderLegend());
  }

  private renderMasterItem(ws: WorkspaceInfo, index: number): HTMLElement {
    const item = document.createElement('div');
    item.className = 'ws-master-item';
    item.tabIndex = 0;
    item.dataset.wsIndex = String(index);

    if (index === this.focusedCardIndex) {
      item.classList.add('ws-master-focused');
    }

    const isInactive = !ws.taskTitle;
    const isInvalid = this.statusErrors.has(ws.id);

    if (isInactive) {
      item.classList.add('ws-inactive');
    }
    if (isInvalid) {
      item.classList.add('ws-invalid');
    }

    // Status dot
    const dot = document.createElement('div');
    dot.className = 'ws-status-dot';
    if (isInvalid) {
      dot.classList.add('error');
    } else if (isInactive) {
      dot.classList.add('pending');
    } else {
      const cached = this.statusCache.get(ws.id);
      if (cached) {
        const hasModifications = cached.some((r) => r.modified > 0 || r.untracked > 0);
        dot.classList.add(hasModifications ? 'modified' : 'clean');
      }
    }
    item.appendChild(dot);

    // Task name
    const taskName = document.createElement('div');
    taskName.className = 'ws-task-name';
    taskName.textContent = isInactive ? '(no active task)' : ws.taskTitle;
    item.appendChild(taskName);

    // Workspace ID
    const id = document.createElement('span');
    id.className = 'ws-id';
    id.textContent = ws.id;
    item.appendChild(id);

    // Click to focus
    item.addEventListener('click', () => {
      this.focusedCardIndex = index;
      this.updateCardFocus();
    });

    return item;
  }

  private renderDetailPanel(ws: WorkspaceInfo): HTMLElement {
    const detail = document.createElement('div');
    detail.className = 'ws-detail';

    const isInactive = !ws.taskTitle;
    const isInvalid = this.statusErrors.has(ws.id);
    const cached = this.statusCache.get(ws.id);

    // Title
    const title = document.createElement('h2');
    title.className = 'ws-detail-title';
    if (isInactive) {
      title.textContent = ws.id;
      title.classList.add('inactive');
    } else {
      title.textContent = ws.taskTitle;
    }
    detail.appendChild(title);

    // Meta row
    const meta = document.createElement('div');
    meta.className = 'ws-detail-meta';

    const detailId = document.createElement('span');
    detailId.className = 'ws-detail-id';
    detailId.textContent = ws.id;
    meta.appendChild(detailId);

    const repoCount = document.createElement('span');
    repoCount.className = 'ws-detail-repo-count';
    repoCount.textContent = `${ws.repos.length} ${ws.repos.length === 1 ? 'repo' : 'repos'}`;
    meta.appendChild(repoCount);

    if (isInvalid) {
      const badge = document.createElement('span');
      badge.className = 'ws-detail-badge error';
      badge.textContent = 'error';
      meta.appendChild(badge);
    } else if (isInactive) {
      const badge = document.createElement('span');
      badge.className = 'ws-detail-badge inactive';
      badge.textContent = 'inactive';
      meta.appendChild(badge);
    } else if (cached) {
      const statusSpan = document.createElement('span');
      const totalModified = cached.reduce((sum, r) => sum + r.modified, 0);
      const totalUntracked = cached.reduce((sum, r) => sum + r.untracked, 0);

      if (totalModified === 0 && totalUntracked === 0) {
        statusSpan.className = 'ws-detail-status clean';
        statusSpan.textContent = 'all clean';
      } else {
        statusSpan.className = 'ws-detail-status modified';
        const parts: string[] = [];
        if (totalModified > 0) parts.push(`${totalModified} modified`);
        if (totalUntracked > 0) parts.push(`${totalUntracked} untracked`);
        statusSpan.textContent = parts.join(', ');
      }
      meta.appendChild(statusSpan);
    }

    detail.appendChild(meta);

    // Error message block (for invalid workspaces)
    if (isInvalid) {
      const errorBlock = document.createElement('div');
      errorBlock.className = 'ws-detail-error';
      errorBlock.textContent = this.statusErrors.get(ws.id)!;
      detail.appendChild(errorBlock);
    }

    // Open button
    const openBtn = document.createElement('button');
    openBtn.className = 'ws-open-btn';
    openBtn.textContent = 'Open Workspace';
    openBtn.addEventListener('click', () => {
      this.onOpenWorkspace(ws.path, ws.taskTitle, ws.repos);
    });
    detail.appendChild(openBtn);

    // Repo table
    if (cached) {
      detail.appendChild(this.renderDetailRepoTable(cached));
    } else {
      const loading = document.createElement('div');
      loading.className = 'ws-detail-loading';
      loading.textContent = 'Loading...';
      detail.appendChild(loading);
    }

    return detail;
  }

  private renderDetailRepoTable(statuses: RepoStatus[]): HTMLElement {
    const table = document.createElement('table');
    table.className = 'ws-detail-repo-table';

    // Header
    const thead = document.createElement('thead');
    const headerRow = document.createElement('tr');
    for (const label of ['Repo', 'Branch', 'Status']) {
      const th = document.createElement('th');
      th.textContent = label;
      headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    table.appendChild(thead);

    // Body
    const tbody = document.createElement('tbody');
    for (const repo of statuses) {
      const row = document.createElement('tr');

      // Repo name
      const nameCell = document.createElement('td');
      nameCell.className = 'col-name';
      nameCell.textContent = repo.name;
      row.appendChild(nameCell);

      // Branch
      const branchCell = document.createElement('td');
      branchCell.className = 'col-branch';
      if (repo.isDetached) {
        branchCell.classList.add('detached');
      }
      branchCell.textContent = repo.branch;
      row.appendChild(branchCell);

      // Status badges
      const statusCell = document.createElement('td');
      const statusContainer = document.createElement('div');
      statusContainer.className = 'col-status';

      if (repo.modified === 0 && repo.untracked === 0) {
        const badge = document.createElement('span');
        badge.className = 'badge badge-clean';
        badge.textContent = '✓ clean';
        statusContainer.appendChild(badge);
      } else {
        if (repo.modified > 0) {
          const badge = document.createElement('span');
          badge.className = 'badge badge-modified';
          badge.textContent = `~${repo.modified} modified`;
          statusContainer.appendChild(badge);
        }
        if (repo.untracked > 0) {
          const badge = document.createElement('span');
          badge.className = 'badge badge-untracked';
          badge.textContent = `+${repo.untracked} untracked`;
          statusContainer.appendChild(badge);
        }
      }

      statusCell.appendChild(statusContainer);
      row.appendChild(statusCell);
      tbody.appendChild(row);
    }
    table.appendChild(tbody);

    return table;
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
      this.statusErrors.clear();
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
