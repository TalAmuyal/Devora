/**
 * Settings Hub: a tab-covering overlay page (master/detail split, mirroring the Workspace Hub) for listing profiles, switching the active one, creating/registering new profiles (inline form behind a pinned "New Profile…" master row), and deleting profiles from the registry.
 * DOM: `div.settings-hub`.
 *
 * q/Esc never reach this page directly — KeyboardShortcuts routes user dismissal through the overlay's onUserDismiss override, which main.ts wires to "return to the Workspace Hub".
 */

import { invoke } from '../invoke';
import { createBadge } from '../ui/components/Badge';
import { createStatusDot } from '../ui/components/StatusDot';
import { createKeyboardHintBar } from '../ui/components/KeyboardHintBar';
import { showConfirmationDialog } from '../ui/components/ConfirmationDialog';
import { createTableShell } from '../ui/components/TableShell';
import { isEditableElementFocused } from '../ui/focus';
import { pluralize } from '../ui/format';
import { createProfileForm } from './ProfileForm';
import { createClaudeConfigCard } from '../ui/components/ClaudeConfigCard';
import { createSettingsCard } from '../ui/components/SettingsCard';
import { ProfileInfo, RepoInfo, WorkspaceInfo } from './types';

/** A row in the master list: the global User Defaults, a profile, or the New Profile form. */
type MasterRow =
  | { kind: 'user-defaults' }
  | { kind: 'profile'; profile: ProfileInfo }
  | { kind: 'new' };

interface ProfileDetail {
  repos: RepoInfo[];
  workspaceCount: number;
  activeTaskCount: number;
}

export type SettingsHubView = 'list' | 'new';

export interface SettingsHubCallbacks {
  getActiveProfilePath: () => string | undefined;
  setActiveProfilePath: (path: string | null) => void;
  /** Open session tabs bound to the given profile (blockers for deletion). */
  getOpenSessionsForProfile: (profilePath: string) => ReadonlyArray<{ title: string }>;
  /** Leave the Settings Hub (main.ts reopens the Workspace Hub). */
  onClose: () => void;
  /** Clone a repo into the given profile; `onDone` is called once it lands so the detail can refresh. */
  onCloneRepo: (profilePath: string, onDone: () => void) => void;
}

export class SettingsHub {
  private containerEl: HTMLElement;
  private callbacks: SettingsHubCallbacks;

  private profiles: ProfileInfo[] = [];
  private loaded = false;
  /** Index into `rows()`: 0 = User Defaults, then the profiles, then the New Profile row. */
  private focusedIndex = 0;
  private detailCache: Map<string, ProfileDetail> = new Map();
  private detailSeq = 0;

  private keyHandler = (e: KeyboardEvent) => this.handleKeyDown(e);

  constructor(callbacks: SettingsHubCallbacks) {
    this.callbacks = callbacks;
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'settings-hub';
  }

  getElement(): HTMLElement {
    return this.containerEl;
  }

  /** The ordered master rows: User Defaults (pinned top), the profiles, then New Profile (pinned bottom). */
  private rows(): MasterRow[] {
    return [
      { kind: 'user-defaults' },
      ...this.profiles.map((profile): MasterRow => ({ kind: 'profile', profile })),
      { kind: 'new' },
    ];
  }

  async load(view: SettingsHubView = 'list'): Promise<void> {
    window.addEventListener('keydown', this.keyHandler, true);
    this.loaded = false;
    this.profiles = [];
    this.detailCache.clear();
    this.focusedIndex = 0;
    this.render();

    try {
      this.profiles = await invoke<ProfileInfo[]>('list_profiles');
    } catch (_) {
      // invoke already surfaced the error
    }
    this.loaded = true;

    if (view === 'new') {
      this.focusedIndex = this.rows().length - 1; // pinned New Profile row
    } else {
      const active = this.callbacks.getActiveProfilePath();
      const activeIndex = this.profiles.findIndex((p) => p.path === active);
      // +1 to skip the leading User Defaults row; fall back to User Defaults (0) when none.
      this.focusedIndex = activeIndex >= 0 ? activeIndex + 1 : 0;
    }

    this.render();
    void this.loadFocusedDetail();
  }

  unload(): void {
    window.removeEventListener('keydown', this.keyHandler, true);
    this.detailSeq++;
    this.profiles = [];
    this.loaded = false;
    this.detailCache.clear();
    this.focusedIndex = 0;
    this.containerEl.innerHTML = '';
  }

  // --- Keyboard ---

  private handleKeyDown(e: KeyboardEvent): void {
    // Let the config card own its own keys (its segmented buttons / chips / dropdown are focusable but not "editable", so they'd otherwise trigger j/k/d/n); this window listener is capture-phase, so the card can't pre-empt it — gate it here.
    const target = e.target;
    if (
      isEditableElementFocused() ||
      (target instanceof Element && target.closest('.claude-config-card'))
    ) {
      return;
    }
    if (!this.loaded) {
      return;
    }

    const rows = this.rows();
    const lastIndex = rows.length - 1; // pinned New Profile… row

    switch (e.key) {
      case 'j':
      case 'ArrowDown':
        e.preventDefault();
        e.stopPropagation();
        this.setFocus(Math.min(this.focusedIndex + 1, lastIndex));
        return;
      case 'k':
      case 'ArrowUp':
        e.preventDefault();
        e.stopPropagation();
        this.setFocus(Math.max(this.focusedIndex - 1, 0));
        return;
      case 'Enter': {
        e.preventDefault();
        e.stopPropagation();
        const row = rows[this.focusedIndex];
        if (row?.kind === 'profile') {
          this.setActiveAndClose(row.profile);
        } else if (row?.kind === 'new') {
          // The form is already in the detail panel; focus it.
          this.focusForm();
        }
        return;
      }
      case 'n':
        e.preventDefault();
        e.stopPropagation();
        this.setFocus(lastIndex);
        this.focusForm();
        return;
      case 'd': {
        e.preventDefault();
        e.stopPropagation();
        const row = rows[this.focusedIndex];
        if (row?.kind === 'profile') {
          void this.handleDelete(row.profile);
        }
        return;
      }
    }
  }

  private setFocus(index: number): void {
    if (index === this.focusedIndex) return;
    this.focusedIndex = index;
    this.render();
    void this.loadFocusedDetail();
  }

  private focusForm(): void {
    const nameInput = this.containerEl.querySelector<HTMLInputElement>('.pm-form-name');
    nameInput?.focus();
  }

  // --- Actions ---

  private setActiveAndClose(profile: ProfileInfo): void {
    this.callbacks.setActiveProfilePath(profile.path);
    this.callbacks.onClose();
  }

  private async handleDelete(profile: ProfileInfo): Promise<void> {
    const blockers = this.callbacks.getOpenSessionsForProfile(profile.path);
    if (blockers.length > 0) {
      const body = document.createElement('div');
      const intro = document.createElement('p');
      intro.textContent = 'These open sessions are using it:';
      intro.style.margin = '0 0 8px 0';
      body.appendChild(intro);
      const list = document.createElement('ul');
      list.style.margin = '0 0 8px 0';
      list.style.paddingLeft = '20px';
      for (const session of blockers) {
        const li = document.createElement('li');
        li.textContent = session.title;
        list.appendChild(li);
      }
      body.appendChild(list);
      const hint = document.createElement('p');
      hint.textContent = 'Close them first, then delete the profile.';
      hint.style.margin = '0';
      body.appendChild(hint);

      await showConfirmationDialog({
        title: `Cannot delete profile "${profile.name}"`,
        body,
        confirmLabel: 'OK',
        hideCancel: true,
      });
      return;
    }

    const confirmed = await showConfirmationDialog({
      title: `Delete profile "${profile.name}"?`,
      body:
        `This removes the profile from Devora's registry only. ` +
        `The directory ${profile.path} and everything in it remains on disk.`,
      confirmLabel: 'Delete Profile',
    });
    if (!confirmed) return;

    try {
      await invoke('unregister_profile', { path: profile.path });
    } catch (_) {
      // invoke already surfaced the error
      return;
    }

    const wasActive = this.callbacks.getActiveProfilePath() === profile.path;
    this.detailCache.delete(profile.path);
    try {
      this.profiles = await invoke<ProfileInfo[]>('list_profiles');
    } catch (_) {
      this.profiles = [];
    }

    if (wasActive) {
      this.callbacks.setActiveProfilePath(this.profiles[0]?.path ?? null);
    }
    if (this.profiles.length === 0) {
      // Back to the hub, which now shows the first-run welcome.
      this.callbacks.onClose();
      return;
    }
    this.focusedIndex = Math.min(this.focusedIndex, this.rows().length - 1);
    this.render();
    void this.loadFocusedDetail();
  }

  /** After a repo is cloned into `profilePath`, re-fetch profiles (so the repo-count badge updates) and the detail (repo table). */
  private async refreshAfterClone(profilePath: string): Promise<void> {
    this.detailCache.delete(profilePath);
    try {
      this.profiles = await invoke<ProfileInfo[]>('list_profiles');
    } catch (_) {
      // invoke already surfaced the error
    }
    this.render();
    void this.loadFocusedDetail();
  }

  private async loadFocusedDetail(): Promise<void> {
    const row = this.rows()[this.focusedIndex];
    if (row?.kind !== 'profile') return;
    const profile = row.profile;
    if (this.detailCache.has(profile.path)) return;

    const seq = ++this.detailSeq;
    try {
      const [repos, workspaces] = await Promise.all([
        invoke<RepoInfo[]>('get_registered_repos', { profilePath: profile.path }),
        invoke<WorkspaceInfo[]>('list_workspaces', { profilePath: profile.path }),
      ]);
      if (seq !== this.detailSeq) return;
      this.detailCache.set(profile.path, {
        repos,
        workspaceCount: workspaces.length,
        activeTaskCount: workspaces.filter((w) => w.active).length,
      });
      const current = this.rows()[this.focusedIndex];
      if (current?.kind === 'profile' && current.profile.path === profile.path) {
        this.updateDetailPanel();
      }
    } catch (_) {
      // invoke already surfaced the error; the detail panel stays in its loading state
    }
  }

  // --- Rendering ---

  private render(): void {
    this.containerEl.innerHTML = '';
    this.containerEl.appendChild(this.renderHeader());
    this.containerEl.appendChild(this.renderSplitPanel());
    this.containerEl.appendChild(
      createKeyboardHintBar({
        hints: [
          { keys: 'j/k', description: 'navigate' },
          { keys: 'Enter', description: 'set active' },
          { keys: 'n', description: 'new profile' },
          { keys: 'd', description: 'delete' },
          { keys: 'q/Esc', description: 'back to hub' },
        ],
      }),
    );
  }

  private renderHeader(): HTMLElement {
    const header = document.createElement('div');
    header.className = 'page-header';

    const title = document.createElement('span');
    title.className = 'page-header-title';
    title.textContent = 'Devora';
    const crumb = document.createElement('span');
    crumb.className = 'pm-header-crumb';
    crumb.textContent = ' › Settings';
    title.appendChild(crumb);
    header.appendChild(title);

    return header;
  }

  private renderSplitPanel(): HTMLElement {
    const split = document.createElement('div');
    split.className = 'pm-split';

    // Master panel (left)
    const masterPanel = document.createElement('div');
    masterPanel.className = 'pm-master-panel';
    const masterList = document.createElement('div');
    masterList.className = 'pm-master-list';

    if (!this.loaded) {
      const placeholder = document.createElement('div');
      placeholder.className = 'panel-loading-placeholder';
      placeholder.textContent = 'Loading profiles...';
      masterList.appendChild(placeholder);
    } else {
      const activePath = this.callbacks.getActiveProfilePath();
      this.rows().forEach((row, i) => {
        if (row.kind === 'user-defaults') {
          masterList.appendChild(this.renderUserDefaultsRow(i));
        } else if (row.kind === 'profile') {
          masterList.appendChild(
            this.renderMasterItem(row.profile, i, row.profile.path === activePath),
          );
        } else {
          masterList.appendChild(this.renderNewProfileRow(i));
        }
      });
    }

    masterPanel.appendChild(masterList);
    split.appendChild(masterPanel);

    // Detail panel (right)
    const detailPanel = document.createElement('div');
    detailPanel.className = 'pm-detail-panel';
    const accent = document.createElement('div');
    accent.className = 'panel-accent';
    detailPanel.appendChild(accent);
    const detailContent = document.createElement('div');
    detailContent.className = 'pm-detail-content';
    if (this.loaded) {
      detailContent.appendChild(this.renderDetail());
    }
    detailPanel.appendChild(detailContent);
    split.appendChild(detailPanel);

    return split;
  }

  private renderMasterItem(profile: ProfileInfo, index: number, isActive: boolean): HTMLElement {
    const item = document.createElement('div');
    item.className = 'pm-master-item';
    if (index === this.focusedIndex) {
      item.classList.add('pm-master-focused');
    }

    item.appendChild(createStatusDot(isActive ? 'clean' : 'pending'));

    const name = document.createElement('div');
    name.className = 'pm-name';
    name.textContent = profile.name;
    item.appendChild(name);

    const meta = document.createElement('span');
    meta.className = 'pm-meta';
    meta.textContent = pluralize(profile.repoCount, 'repo');
    item.appendChild(meta);

    item.addEventListener('click', () => this.setFocus(index));
    return item;
  }

  private renderUserDefaultsRow(index: number): HTMLElement {
    return this.renderFixedRow(index, 'pm-user-defaults-row', 'pm-user-defaults-glyph', '⚙', 'User Defaults');
  }

  private renderNewProfileRow(index: number): HTMLElement {
    return this.renderFixedRow(index, 'pm-new-row', 'pm-new-row-plus', '＋', 'New Profile…');
  }

  /** A pinned master row (User Defaults / New Profile): a leading glyph plus a label. */
  private renderFixedRow(
    index: number,
    rowClass: string,
    glyphClass: string,
    glyphText: string,
    label: string,
  ): HTMLElement {
    const item = document.createElement('div');
    item.className = `pm-master-item ${rowClass}`;
    if (index === this.focusedIndex) {
      item.classList.add('pm-master-focused');
    }

    const glyph = document.createElement('span');
    glyph.className = glyphClass;
    glyph.textContent = glyphText;
    item.appendChild(glyph);

    const labelEl = document.createElement('div');
    labelEl.className = 'pm-name';
    labelEl.textContent = label;
    item.appendChild(labelEl);

    item.addEventListener('click', () => this.setFocus(index));
    return item;
  }

  private renderDetail(): HTMLElement {
    const row = this.rows()[this.focusedIndex];
    if (!row) {
      return document.createElement('div');
    }
    switch (row.kind) {
      case 'user-defaults':
        return this.renderUserDefaults();
      case 'new':
        return this.renderNewProfileForm();
      case 'profile':
        return this.renderProfileDetail(row.profile);
    }
  }

  private renderUserDefaults(): HTMLElement {
    const wrap = document.createElement('div');
    wrap.className = 'pm-detail';

    const title = document.createElement('h2');
    title.className = 'pm-detail-title';
    title.textContent = 'User Defaults';
    wrap.appendChild(title);

    const desc = document.createElement('div');
    desc.className = 'pm-detail-path';
    desc.textContent = 'Applied to every profile unless a profile overrides a setting.';
    wrap.appendChild(desc);

    wrap.appendChild(createClaudeConfigCard({ profilePath: null }));
    return wrap;
  }

  private updateDetailPanel(): void {
    const contentEl = this.containerEl.querySelector('.pm-detail-content');
    if (!contentEl) return;
    contentEl.innerHTML = '';
    contentEl.appendChild(this.renderDetail());
  }

  private renderNewProfileForm(): HTMLElement {
    const wrap = document.createElement('div');
    wrap.className = 'pm-detail';

    const title = document.createElement('h2');
    title.className = 'pm-detail-title';
    title.textContent = 'New Profile';
    wrap.appendChild(title);

    const formHandle = createProfileForm({
      onRegistered: (profile) => {
        this.callbacks.setActiveProfilePath(profile.path);
        this.callbacks.onClose();
      },
      onCancel: () => {
        const active = this.callbacks.getActiveProfilePath();
        const activeIndex = this.profiles.findIndex((p) => p.path === active);
        this.setFocus(Math.max(activeIndex, 0));
      },
    });
    wrap.appendChild(formHandle.element);
    return wrap;
  }

  private renderProfileDetail(profile: ProfileInfo): HTMLElement {
    const detail = document.createElement('div');
    detail.className = 'pm-detail';

    const title = document.createElement('h2');
    title.className = 'pm-detail-title';
    title.textContent = profile.name;
    detail.appendChild(title);

    const path = document.createElement('div');
    path.className = 'pm-detail-path';
    path.textContent = profile.path;
    detail.appendChild(path);

    const cached = this.detailCache.get(profile.path);
    const isActive = this.callbacks.getActiveProfilePath() === profile.path;

    const badges = document.createElement('div');
    badges.className = 'pm-detail-badges';
    if (isActive) {
      badges.appendChild(createBadge('active', 'clean'));
    }
    badges.appendChild(createBadge(pluralize(profile.repoCount, 'repo'), 'inactive'));
    if (cached) {
      badges.appendChild(
        createBadge(pluralize(cached.workspaceCount, 'workspace'), 'inactive'),
      );
      badges.appendChild(
        createBadge(pluralize(cached.activeTaskCount, 'active task'), 'untracked'),
      );
    }
    detail.appendChild(badges);

    const actionRow = document.createElement('div');
    actionRow.className = 'pm-action-row';
    const setActiveBtn = document.createElement('button');
    setActiveBtn.className = 'pm-set-active-btn';
    setActiveBtn.textContent = 'Set Active';
    setActiveBtn.disabled = isActive;
    setActiveBtn.addEventListener('click', () => this.setActiveAndClose(profile));
    actionRow.appendChild(setActiveBtn);
    const cloneBtn = document.createElement('button');
    cloneBtn.className = 'pm-clone-btn';
    cloneBtn.textContent = 'Clone Repo';
    cloneBtn.addEventListener('click', () =>
      this.callbacks.onCloneRepo(profile.path, () => void this.refreshAfterClone(profile.path)),
    );
    actionRow.appendChild(cloneBtn);
    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'pm-delete-btn';
    deleteBtn.textContent = 'Delete';
    deleteBtn.addEventListener('click', () => void this.handleDelete(profile));
    actionRow.appendChild(deleteBtn);
    detail.appendChild(actionRow);

    const reposCard = createSettingsCard('Repos');
    if (!cached) {
      const loading = document.createElement('div');
      loading.className = 'panel-detail-loading';
      loading.textContent = 'Loading...';
      reposCard.appendChild(loading);
    } else if (cached.repos.length === 0) {
      const none = document.createElement('div');
      none.className = 'panel-detail-loading';
      none.textContent = `No repos yet — clone one into ${profile.path}/repos/`;
      reposCard.appendChild(none);
    } else {
      reposCard.appendChild(this.renderRepoTable(cached.repos));
    }
    detail.appendChild(reposCard);

    detail.appendChild(createClaudeConfigCard({ profilePath: profile.path }));

    return detail;
  }

  private renderRepoTable(repos: RepoInfo[]): HTMLElement {
    const { table, tbody } = createTableShell(['Repo', 'Source', 'Path'], 'pm-repo-table');

    for (const repo of repos) {
      const row = document.createElement('tr');

      const nameCell = document.createElement('td');
      nameCell.className = 'col-name';
      nameCell.textContent = repo.name;
      row.appendChild(nameCell);

      const sourceCell = document.createElement('td');
      sourceCell.className = 'pm-repo-source';
      sourceCell.textContent = repo.source;
      row.appendChild(sourceCell);

      const pathCell = document.createElement('td');
      pathCell.className = 'pm-repo-path';
      pathCell.textContent = repo.path;
      row.appendChild(pathCell);

      tbody.appendChild(row);
    }

    return table;
  }
}
