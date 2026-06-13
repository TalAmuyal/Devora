import { invoke } from '../invoke';
import { createEmptyState } from '../ui/components/EmptyState';
import { createStatusDot } from '../ui/components/StatusDot';
import { createBadge } from '../ui/components/Badge';
import { createSegmentedControl } from '../ui/components/SegmentedControl';
import { createSearchInput, SearchInputHandle } from '../ui/components/SearchInput';
import { createKeyboardHintBar } from '../ui/components/KeyboardHintBar';
import { showConfirmationDialog } from '../ui/components/ConfirmationDialog';
import { createToast, ToastHandle } from '../ui/components/Toast';
import { createDropdownMenu, DropdownItem } from '../ui/components/DropdownMenu';
import { createTableShell } from '../ui/components/TableShell';
import { isEditableElementFocused } from '../ui/focus';
import { pluralize } from '../ui/format';
import { createProfileForm } from './ProfileForm';
import { ProfileInfo, RepoInfo, WorkspaceInfo } from './types';

// "Refreshing…" stays visible at least this long, even when the reload is instant
const REFRESH_TOAST_MIN_MS = 1000;

// How long the "Refreshed successfully" confirmation stays visible
const REFRESHED_TOAST_MS = 3000;

// Types matching the Rust command return values
interface RepoStatusTiming {
  totalMs: number;
  gitStatusMs: number;
  gitRevParseMs: number | null;
  spawnOverheadMs: number;
}

interface RepoStatus {
  name: string;
  branch: string;
  isDetached: boolean;
  modified: number;
  untracked: number;
  timing: RepoStatusTiming | null;
}

interface WorkspaceStatusResult {
  statuses: RepoStatus[];
  threadSpawnMs: number;
  threadJoinMs: number;
  handlerTotalMs: number;
}

interface WorkspaceStatusInput {
  workspacePath: string;
  repoNames: string[];
}

interface BatchWorkspaceStatusResult {
  workspaceStatuses: Array<{
    workspacePath: string;
    statuses: RepoStatus[];
    error: string | null;
  }>;
  handlerTotalMs: number;
  threadSpawnMs: number;
  threadJoinMs: number;
}

interface HubProfilingReport {
  timestamp: string;
  profileName: string;
  profilePath: string;
  workspaceCount: number;
  phases: {
    listProfiles: { startMs: number; durationMs: number };
    listWorkspaces: { startMs: number; durationMs: number };
    render: { startMs: number; durationMs: number };
    totalLoad: { startMs: number; durationMs: number };
  };
  workspaceStatuses: Array<{
    wsId: string;
    repoCount: number;
    startMs: number;
    durationMs: number;
    error?: string;
    handlerTotalMs?: number;
    threadSpawnMs?: number;
    threadJoinMs?: number;
    ipcOverheadMs?: number;
    repoTimings?: Array<{
      name: string;
      totalMs: number;
      gitStatusMs: number;
      gitRevParseMs: number | null;
      spawnOverheadMs: number;
    }>;
  }>;
  batchTiming?: {
    startMs: number;
    durationMs: number;
    handlerTotalMs: number;
    threadSpawnMs: number;
    threadJoinMs: number;
    ipcOverheadMs: number;
    workspaceCount: number;
    totalRepoCount: number;
  };
}

type CategoryFilter = 'active' | 'inactive' | 'all';

export class WorkspaceHub {
  private containerEl: HTMLElement;
  private onOpenWorkspace: (path: string, title: string, repos: string[]) => void;
  // Hand off a new task: the controller opens a tab + progress overlay and runs creation asynchronously.
  private onStartTaskCreation: (taskName: string, repoPaths: string[]) => void;
  private onClose: () => void;
  private onOpenProfileManager: (view: 'list' | 'new') => void;

  private profiles: ProfileInfo[] = [];
  private activeProfilePath: string | null = null;
  // Whether the last completed load found zero profiles.
  // Deliberately NOT reset in unload(): main.ts consults it (via isZeroProfileLocked) while other overlays cover the hub, e.g. to decide where q/Esc should land.
  private zeroProfiles = false;
  private workspaces: WorkspaceInfo[] = [];
  private searchFilter: string = '';
  private categoryFilter: CategoryFilter = 'active';

  private statusCache: Map<string, RepoStatus[]> = new Map();
  private statusErrors: Map<string, string> = new Map();

  private showNewForm = false;
  private refreshToast: ToastHandle | null = null;
  private refreshSeq = 0;
  private availableRepos: RepoInfo[] = [];
  private focusedCardIndex = -1;
  private showCheatsheet = false;
  private profilesLoaded = false;
  private workspacesLoaded = false;

  private profilingData: HubProfilingReport | null = null;
  private profilingT0: number = 0;
  private profilingSaved: boolean = false;
  private profilingError: boolean = false;

  private searchHandle: SearchInputHandle | null = null;
  private masterListEl: HTMLElement | null = null;

  private keyHandler = (e: KeyboardEvent) => this.handleKeyDown(e);

  constructor(
    onOpenWorkspace: (path: string, title: string, repos: string[]) => void,
    onStartTaskCreation: (taskName: string, repoPaths: string[]) => void,
    onClose: () => void,
    onOpenProfileManager: (view: 'list' | 'new') => void,
  ) {
    this.onOpenWorkspace = onOpenWorkspace;
    this.onStartTaskCreation = onStartTaskCreation;
    this.onClose = onClose;
    this.onOpenProfileManager = onOpenProfileManager;
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'ws-hub';
  }

  getElement(): HTMLElement {
    return this.containerEl;
  }

  getActiveProfilePath(): string | undefined {
    return this.activeProfilePath ?? undefined;
  }

  /**
   * Switch the active profile from outside the hub (Profile Manager, command palette).
   * No render — the next load() picks the new profile up.
   */
  setActiveProfilePath(path: string | null): void {
    this.activeProfilePath = path;
    this.statusCache.clear();
    this.statusErrors.clear();
  }

  /** True when the last load found zero profiles: the hub shows the first-run welcome and must not be dismissible (there is nothing behind it). */
  isZeroProfileLocked(): boolean {
    return this.zeroProfiles;
  }

  /** Single entry point for user-initiated dismissal (q/Esc/Ctrl+S, routed through the overlay's onUserDismiss override): closes the cheatsheet if open, refuses while zero-profile locked, otherwise closes the hub. */
  handleUserDismiss(): void {
    if (this.showCheatsheet) {
      this.showCheatsheet = false;
      this.render();
      return;
    }
    if (this.isZeroProfileLocked()) {
      return;
    }
    this.onClose();
  }

  async load(): Promise<void> {
    this.profilingT0 = performance.now();
    this.profilingSaved = false;
    this.profilingError = false;

    window.addEventListener('keydown', this.keyHandler, true);
    this.profilesLoaded = false;
    this.workspacesLoaded = false;

    const renderStart = performance.now();
    this.render();
    const renderDuration = performance.now() - renderStart;

    let listProfilesDuration: number;
    let listProfilesStart: number;
    let listWorkspacesDuration: number;
    let listWorkspacesStart: number;

    if (this.activeProfilePath) {
      listProfilesStart = performance.now();
      listWorkspacesStart = listProfilesStart;
      try {
        const [profiles] = await Promise.all([
          invoke<ProfileInfo[]>('list_profiles'),
          this.loadWorkspaces(),
        ]);
        this.profiles = profiles;
        this.zeroProfiles = profiles.length === 0;
        // The remembered profile may have been deleted since the last load.
        if (!profiles.some((p) => p.path === this.activeProfilePath)) {
          this.activeProfilePath = profiles[0]?.path ?? null;
          this.statusCache.clear();
          this.statusErrors.clear();
          this.workspaces = [];
          if (this.activeProfilePath) {
            await this.loadWorkspaces();
          }
        }
      } catch (_) {
        // invoke already surfaced the error; keep the previous zeroProfiles verdict — a transient listing failure must not lock the hub.
      }
      listProfilesDuration = performance.now() - listProfilesStart;
      listWorkspacesDuration = listProfilesDuration;
      this.profilesLoaded = true;
      this.updateHeader();
    } else {
      listProfilesStart = performance.now();
      try {
        this.profiles = await invoke<ProfileInfo[]>('list_profiles');
        this.profilesLoaded = true;
        this.zeroProfiles = this.profiles.length === 0;
        if (this.profiles.length > 0) {
          this.activeProfilePath = this.profiles[0].path;
        }
        this.updateHeader();
      } catch (_) {
        // invoke already surfaced the error
        this.profilesLoaded = true;
      }
      listProfilesDuration = performance.now() - listProfilesStart;

      listWorkspacesStart = performance.now();
      if (this.activeProfilePath) {
        await this.loadWorkspaces();
      }
      listWorkspacesDuration = performance.now() - listWorkspacesStart;
    }

    this.workspacesLoaded = true;

    const totalLoadDuration = performance.now() - this.profilingT0;

    const activeProfile = this.profiles.find((p) => p.path === this.activeProfilePath);
    this.profilingData = {
      timestamp: new Date().toISOString(),
      profileName: activeProfile?.name ?? '',
      profilePath: this.activeProfilePath ?? '',
      workspaceCount: this.workspaces.length,
      phases: {
        listProfiles: {
          startMs: listProfilesStart - this.profilingT0,
          durationMs: listProfilesDuration,
        },
        listWorkspaces: {
          startMs: listWorkspacesStart - this.profilingT0,
          durationMs: listWorkspacesDuration,
        },
        render: {
          startMs: renderStart - this.profilingT0,
          durationMs: renderDuration,
        },
        totalLoad: {
          startMs: 0,
          durationMs: totalLoadDuration,
        },
      },
      workspaceStatuses: [],
    };

    this.render();
    this.preloadAllStatuses();
  }

  async refresh(): Promise<void> {
    // Latest press wins: bump the token so any in-flight sequence aborts at its next checkpoint, and clear existing toasts so the new one replaces them without overlap
    const seq = ++this.refreshSeq;
    this.removeAllToasts();

    const refreshingToast = createToast('Refreshing…');
    this.refreshToast = refreshingToast;
    const startedAt = performance.now();
    this.statusCache.clear();
    this.statusErrors.clear();

    let failed = false;
    try {
      try {
        this.profiles = await invoke<ProfileInfo[]>('list_profiles');
        const error = await this.loadWorkspaces();
        if (error) {
          failed = true;
        }
      } catch (_) {
        // invoke already surfaced the error
        failed = true;
      }
      // Superseded by a newer press or the hub closing — stop before touching the UI.
      if (seq !== this.refreshSeq) return;
      this.renderAfterWorkspaceChange();

      // Keep "Refreshing…" up for at least the minimum, then fade it fully out.
      const elapsed = performance.now() - startedAt;
      if (elapsed < REFRESH_TOAST_MIN_MS) {
        await new Promise((resolve) => setTimeout(resolve, REFRESH_TOAST_MIN_MS - elapsed));
      }
      if (seq !== this.refreshSeq) return;
      await refreshingToast.dismiss();
      if (seq !== this.refreshSeq) return;
      this.refreshToast = null;

      // Success only — a failed refresh already surfaced an error banner.
      if (failed) return;
      const doneToast = createToast('Refreshed successfully');
      this.refreshToast = doneToast;
      await new Promise((resolve) => setTimeout(resolve, REFRESHED_TOAST_MS));
      if (seq !== this.refreshSeq) return;
      await doneToast.dismiss();
      if (seq !== this.refreshSeq) return;
      this.refreshToast = null;
    } finally {
      // Abnormal exit (e.g. a render error) while still the current sequence:
      // make sure this sequence's toast doesn't linger.
      if (seq === this.refreshSeq && this.refreshToast) {
        this.refreshToast.element.remove();
        this.refreshToast = null;
      }
    }
  }

  /** Remove every toast element from the document (used when starting a fresh
   * refresh or tearing the hub down) so none can be orphaned on screen. */
  private removeAllToasts(): void {
    document.querySelectorAll('.toast').forEach((el) => el.remove());
    this.refreshToast = null;
  }

  /**
   * Re-render after the workspace list changed: keep the focused row within
   * range (favouring the last row over jumping to the top), redraw, and kick
   * off a status preload for any newly visible workspaces.
   */
  private renderAfterWorkspaceChange(): void {
    const filtered = this.filteredWorkspaces();
    if (this.focusedCardIndex >= filtered.length) {
      this.focusedCardIndex = filtered.length > 0 ? filtered.length - 1 : -1;
    }
    this.render();
    this.preloadAllStatuses();
  }

  private preloadAllStatuses(): void {
    const inputs: WorkspaceStatusInput[] = this.workspaces
      .filter((ws) => !this.statusCache.has(ws.id))
      .map((ws) => ({ workspacePath: ws.path, repoNames: ws.repos }));

    if (inputs.length === 0) return;

    const batchStartMs = performance.now() - this.profilingT0;
    const batchStartAbs = performance.now();

    invoke<BatchWorkspaceStatusResult>('get_all_workspace_statuses', { workspaces: inputs })
      .then((result) => {
        const batchDurationMs = performance.now() - batchStartAbs;

        for (const wsResult of result.workspaceStatuses) {
          const ws = this.workspaces.find((w) => w.path === wsResult.workspacePath);
          if (!ws) continue;

          if (wsResult.error) {
            this.statusErrors.set(ws.id, wsResult.error);
            this.updateMasterItemError(ws.id);
          } else {
            this.statusCache.set(ws.id, wsResult.statuses);
            this.statusErrors.delete(ws.id);
            this.updateCardStatus(ws.id, wsResult.statuses);
          }

          this.profilingData?.workspaceStatuses.push({
            wsId: ws.id,
            repoCount: ws.repos.length,
            startMs: batchStartMs,
            durationMs: batchDurationMs,
            repoTimings: wsResult.statuses
              .filter((s) => s.timing)
              .map((s) => ({
                name: s.name,
                totalMs: s.timing!.totalMs,
                gitStatusMs: s.timing!.gitStatusMs,
                gitRevParseMs: s.timing!.gitRevParseMs,
                spawnOverheadMs: s.timing!.spawnOverheadMs,
              })),
          });
        }

        if (this.profilingData) {
          this.profilingData.batchTiming = {
            startMs: batchStartMs,
            durationMs: batchDurationMs,
            handlerTotalMs: result.handlerTotalMs,
            threadSpawnMs: result.threadSpawnMs,
            threadJoinMs: result.threadJoinMs,
            ipcOverheadMs: batchDurationMs - result.handlerTotalMs,
            workspaceCount: inputs.length,
            totalRepoCount: inputs.reduce((sum, i) => sum + i.repoNames.length, 0),
          };
        }
      })
      .catch((err) => {
        const errorMsg = err instanceof Error ? err.message : String(err);
        for (const input of inputs) {
          const ws = this.workspaces.find((w) => w.path === input.workspacePath);
          if (!ws) continue;
          this.statusErrors.set(ws.id, errorMsg);
          this.updateMasterItemError(ws.id);
        }
      });
  }

  private updateMasterItemError(wsId: string): void {
    const items = this.containerEl.querySelectorAll('.ws-master-item');
    for (const item of items) {
      if ((item as HTMLElement).dataset.wsIndex === undefined) continue;
      const idEl = item.querySelector('.ws-id');
      if (idEl?.textContent !== wsId) continue;

      item.classList.add('ws-invalid');
      const dot = item.querySelector('.status-dot');
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
      const dot = item.querySelector('.status-dot');
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
    this.refreshSeq++;
    this.removeAllToasts();
    this.showCheatsheet = false;
    this.profilesLoaded = false;
    this.workspacesLoaded = false;
    this.statusCache.clear();
    this.statusErrors.clear();
    this.workspaces = [];
    this.profiles = [];
    this.profilingData = null;
  }

  private handleKeyDown(e: KeyboardEvent): void {
    if (isEditableElementFocused()) {
      return;
    }

    // First-run welcome: only the form is interactive (q/Esc are already swallowed upstream by the overlay's user-dismiss override).
    if (this.isZeroProfileLocked()) {
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
        this.searchHandle?.focus();
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
      case 'n': {
        e.preventDefault();
        e.stopPropagation();
        this.toggleNewForm();
        return;
      }
      case 'R': {
        e.preventDefault();
        e.stopPropagation();
        void this.refresh();
        return;
      }
      case 'P': {
        e.preventDefault();
        e.stopPropagation();
        this.onOpenProfileManager('list');
        return;
      }
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

  private updateMasterList(): void {
    if (!this.masterListEl) return;

    this.masterListEl.innerHTML = '';
    const filtered = this.filteredWorkspaces();

    if (this.focusedCardIndex >= filtered.length) {
      this.focusedCardIndex = filtered.length > 0 ? 0 : -1;
    } else if (this.focusedCardIndex < 0 && filtered.length > 0) {
      this.focusedCardIndex = 0;
    }

    if (!this.workspacesLoaded) {
      const placeholder = document.createElement('div');
      placeholder.className = 'panel-loading-placeholder';
      placeholder.textContent = 'Loading workspaces...';
      this.masterListEl.appendChild(placeholder);
    } else if (filtered.length === 0) {
      this.masterListEl.appendChild(createEmptyState('No workspaces found'));
    } else {
      for (let i = 0; i < filtered.length; i++) {
        this.masterListEl.appendChild(this.renderMasterItem(filtered[i], i));
      }
    }

    this.updateCardFocus();
  }

  private updateDetailPanel(): void {
    const filtered = this.filteredWorkspaces();
    const contentEl = this.containerEl.querySelector('.ws-detail-content');
    if (!contentEl) return;

    contentEl.innerHTML = '';

    if (this.workspacesLoaded && this.focusedCardIndex >= 0 && this.focusedCardIndex < filtered.length) {
      const ws = filtered[this.focusedCardIndex];
      contentEl.appendChild(this.renderDetailPanel(ws));
    }
  }

  /** Loads workspaces for the active profile. Returns an error message on
   * failure (leaving the list empty), or null on success. */
  private async loadWorkspaces(): Promise<string | null> {
    if (!this.activeProfilePath) return null;
    try {
      this.workspaces = await invoke<WorkspaceInfo[]>('list_workspaces', {
        profilePath: this.activeProfilePath,
      });
      return null;
    } catch (e) {
      // invoke already surfaced the error; the message is returned for flow control
      this.workspaces = [];
      return e instanceof Error ? e.message : String(e);
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
    this.searchHandle = null;
    this.masterListEl = null;
    this.containerEl.innerHTML = '';

    if (this.showCheatsheet) {
      this.containerEl.appendChild(this.renderCheatsheet());
      return;
    }

    this.containerEl.appendChild(this.renderHeader());

    if (this.profilesLoaded && this.profiles.length === 0) {
      this.containerEl.appendChild(this.renderWelcome());
      this.containerEl.appendChild(createKeyboardHintBar({
        hints: [
          { keys: 'Tab', description: 'next field' },
          { keys: 'Enter', description: 'create profile' },
        ],
      }));
      return;
    }

    if (this.showNewForm) {
      this.containerEl.appendChild(this.renderNewForm());
    } else {
      this.containerEl.appendChild(this.renderSplitPanel());
    }

    this.containerEl.appendChild(createKeyboardHintBar({
      hints: [
        { keys: 'j/k', description: 'navigate' },
        { keys: 'Enter', description: 'open' },
        { keys: 'f', description: 'filter' },
        { keys: '1/2/3', description: 'active/inactive/all' },
        { keys: 'n', description: 'new task' },
        { keys: 'P', description: 'profiles' },
        { keys: 'R', description: 'refresh' },
        { keys: 'q/Esc', description: 'close' },
        { keys: '?', description: 'all shortcuts' },
      ],
      trailing: this.renderProfilingButton(),
    }));
  }

  /**
   * First-run (or all-profiles-deleted) welcome: a centered card with the profile creation form.
   * Shown instead of the split panel; the hub is not dismissible in this state.
   */
  private renderWelcome(): HTMLElement {
    const wrap = document.createElement('div');
    wrap.className = 'ws-welcome';

    const card = document.createElement('div');
    card.className = 'ws-welcome-card';

    const accent = document.createElement('div');
    accent.className = 'panel-accent';
    card.appendChild(accent);

    const inner = document.createElement('div');
    inner.className = 'ws-welcome-inner';

    const glyph = document.createElement('div');
    glyph.className = 'ws-welcome-glyph';
    glyph.textContent = 'D';
    inner.appendChild(glyph);

    const title = document.createElement('h2');
    title.className = 'ws-welcome-title';
    title.textContent = 'Welcome to Devora';
    inner.appendChild(title);

    const sub = document.createElement('p');
    sub.className = 'ws-welcome-sub';
    sub.textContent =
      'Create your first profile to get started. A profile is an isolated scope ' +
      'with its own repos and workspaces — for example, one for work and one for ' +
      'personal projects.';
    inner.appendChild(sub);

    const form = createProfileForm({
      initialPath: '~/devora',
      onRegistered: (profile) => {
        this.setActiveProfilePath(profile.path);
        void this.load();
      },
    });
    inner.appendChild(form.element);

    card.appendChild(inner);
    wrap.appendChild(card);
    return wrap;
  }

  private renderSplitPanel(): HTMLElement {
    const filtered = this.filteredWorkspaces();

    if (filtered.length > 0 && this.focusedCardIndex < 0) {
      this.focusedCardIndex = 0;
    }

    const split = document.createElement('div');
    split.className = 'ws-split';

    // Master panel (left)
    const masterPanel = document.createElement('div');
    masterPanel.className = 'ws-master-panel';

    const masterHeader = document.createElement('div');
    masterHeader.className = 'ws-master-header';
    this.searchHandle = createSearchInput({
      placeholder: 'Filter...',
      value: this.searchFilter,
      icon: '⌕',
      onInput: (value) => {
        this.searchFilter = value;
        this.updateMasterList();
      },
      onEscape: () => {},
    });
    masterHeader.appendChild(this.searchHandle.element);
    masterHeader.appendChild(createSegmentedControl({
      items: [
        { key: 'active' as const, label: 'Active' },
        { key: 'inactive' as const, label: 'Inactive' },
        { key: 'all' as const, label: 'All' },
      ],
      activeKey: this.categoryFilter,
      onSelect: (key) => {
        this.categoryFilter = key;
        this.render();
      },
    }));
    if (this.profilesLoaded) {
      masterHeader.appendChild(this.renderNewButton());
    }
    masterPanel.appendChild(masterHeader);

    const separator = document.createElement('div');
    separator.className = 'ws-controls-list-separator';
    masterPanel.appendChild(separator);

    const masterList = document.createElement('div');
    masterList.className = 'ws-master-list';
    this.masterListEl = masterList;

    if (!this.workspacesLoaded) {
      const placeholder = document.createElement('div');
      placeholder.className = 'panel-loading-placeholder';
      placeholder.textContent = 'Loading workspaces...';
      masterList.appendChild(placeholder);
    } else if (filtered.length === 0) {
      masterList.appendChild(createEmptyState('No workspaces found'));
    } else {
      for (let i = 0; i < filtered.length; i++) {
        masterList.appendChild(this.renderMasterItem(filtered[i], i));
      }
    }
    masterPanel.appendChild(masterList);

    split.appendChild(masterPanel);

    // Detail panel (right)
    const detailPanel = document.createElement('div');
    detailPanel.className = 'ws-detail-panel';

    const accent = document.createElement('div');
    accent.className = 'panel-accent';
    detailPanel.appendChild(accent);

    const detailContent = document.createElement('div');
    detailContent.className = 'ws-detail-content';

    if (this.workspacesLoaded && this.focusedCardIndex >= 0 && this.focusedCardIndex < filtered.length) {
      detailContent.appendChild(this.renderDetailPanel(filtered[this.focusedCardIndex]));
    }

    detailPanel.appendChild(detailContent);

    split.appendChild(detailPanel);
    return split;
  }

  private updateHeader(): void {
    const header = this.containerEl.querySelector('.page-header');
    if (!header) return;
    header.querySelector('.ws-profile-dropdown')?.remove();
    if (this.profilesLoaded && this.profiles.length > 0) {
      header.appendChild(this.buildProfileDropdown());
    }
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
    let dotVariant: 'clean' | 'modified' | 'pending' | 'error';
    if (isInvalid) {
      dotVariant = 'error';
    } else if (isInactive) {
      dotVariant = 'pending';
    } else {
      const cached = this.statusCache.get(ws.id);
      if (cached) {
        const hasModifications = cached.some((r) => r.modified > 0 || r.untracked > 0);
        dotVariant = hasModifications ? 'modified' : 'clean';
      } else {
        dotVariant = 'pending';
      }
    }
    const dot = createStatusDot(dotVariant);
    item.appendChild(dot);

    // Task name
    const taskName = document.createElement('div');
    taskName.className = 'ws-task-name';
    taskName.textContent = isInactive ? '(inactive)' : ws.taskTitle;
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

  private async handleRemoveTask(ws: WorkspaceInfo): Promise<void> {
    const cached = this.statusCache.get(ws.id);
    const isClean =
      cached !== undefined &&
      cached.every((r) => r.isDetached && r.modified === 0 && r.untracked === 0);

    if (!isClean) {
      const issues: { name: string; reason: string }[] = [];
      if (cached) {
        for (const repo of cached) {
          const reasons: string[] = [];
          if (repo.modified > 0 || repo.untracked > 0) {
            reasons.push('uncommitted changes (will be discarded)');
          }
          if (!repo.isDetached) {
            reasons.push(`on branch '${repo.branch}' (will be detached)`);
          }
          if (reasons.length > 0) {
            issues.push({ name: repo.name, reason: reasons.join('; ') });
          }
        }
      }

      const bodyEl = document.createElement('div');
      const intro = document.createElement('p');
      intro.textContent = 'The following repos need cleanup:';
      intro.style.margin = '0 0 8px 0';
      bodyEl.appendChild(intro);

      const list = document.createElement('ul');
      list.style.margin = '0 0 8px 0';
      list.style.paddingLeft = '20px';
      for (const issue of issues) {
        const li = document.createElement('li');
        const name = document.createElement('strong');
        name.textContent = issue.name;
        li.appendChild(name);
        li.appendChild(document.createTextNode(` — ${issue.reason}`));
        list.appendChild(li);
      }
      bodyEl.appendChild(list);

      const warning = document.createElement('p');
      warning.textContent = 'This cannot be undone.';
      warning.style.margin = '0';
      bodyEl.appendChild(warning);

      const confirmed = await showConfirmationDialog({
        title: `Remove task from "${ws.id}"?`,
        body: bodyEl,
        confirmLabel: 'Remove Task',
      });
      if (!confirmed) return;
    }

    try {
      await invoke('remove_task', { workspacePath: ws.path });
    } catch (_) {
      // invoke already surfaced the error
      return;
    }

    this.statusCache.delete(ws.id);
    this.statusErrors.delete(ws.id);
    await this.loadWorkspaces();
    this.renderAfterWorkspaceChange();
  }

  private async handleDeleteWorkspace(ws: WorkspaceInfo): Promise<void> {
    try {
      await invoke('delete_workspace', { workspacePath: ws.path });
    } catch (_) {
      // invoke already surfaced the error
      return;
    }

    this.statusCache.delete(ws.id);
    this.statusErrors.delete(ws.id);
    await this.loadWorkspaces();
    this.renderAfterWorkspaceChange();
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
    repoCount.textContent = pluralize(ws.repos.length, 'repo');
    meta.appendChild(repoCount);

    if (isInvalid) {
      meta.appendChild(createBadge('error', 'error'));
    } else if (isInactive) {
      meta.appendChild(createBadge('inactive', 'inactive'));
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

    // Action row (Open + Remove Task / Delete)
    const actionRow = document.createElement('div');
    actionRow.className = 'ws-action-row';

    const openBtn = document.createElement('button');
    openBtn.className = 'ws-open-btn';
    openBtn.textContent = 'Open';
    openBtn.addEventListener('click', () => {
      this.onOpenWorkspace(ws.path, ws.taskTitle, ws.repos);
    });
    actionRow.appendChild(openBtn);

    if (ws.active) {
      const removeTaskBtn = document.createElement('button');
      removeTaskBtn.className = 'ws-remove-task-btn';
      removeTaskBtn.textContent = 'Remove Task';
      removeTaskBtn.addEventListener('click', () => this.handleRemoveTask(ws));
      actionRow.appendChild(removeTaskBtn);
    } else {
      const deleteBtn = document.createElement('button');
      deleteBtn.className = 'ws-delete-btn';
      deleteBtn.textContent = 'Delete';
      deleteBtn.addEventListener('click', () => this.handleDeleteWorkspace(ws));
      actionRow.appendChild(deleteBtn);
    }

    detail.appendChild(actionRow);

    // Repo table
    if (cached) {
      detail.appendChild(this.renderDetailRepoTable(cached));
    } else if (!isInvalid) {
      detail.appendChild(this.renderPendingRepoTable(ws.repos));
    }

    return detail;
  }

  private createRepoTableShell(extraClass?: string): { table: HTMLElement; tbody: HTMLElement } {
    const className = 'ws-detail-repo-table' + (extraClass ? ' ' + extraClass : '');
    return createTableShell(['Repo', 'Branch', 'Status'], className);
  }

  private renderDetailRepoTable(statuses: RepoStatus[]): HTMLElement {
    const { table, tbody } = this.createRepoTableShell();

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
        statusContainer.appendChild(createBadge('✓ clean', 'clean'));
      } else {
        if (repo.modified > 0) {
          statusContainer.appendChild(createBadge(`~${repo.modified} modified`, 'modified'));
        }
        if (repo.untracked > 0) {
          statusContainer.appendChild(createBadge(`+${repo.untracked} untracked`, 'untracked'));
        }
      }

      statusCell.appendChild(statusContainer);
      row.appendChild(statusCell);
      tbody.appendChild(row);
    }

    return table;
  }

  private renderPendingRepoTable(repoNames: string[]): HTMLElement {
    const { table, tbody } = this.createRepoTableShell('ws-detail-repo-pending');

    for (const name of repoNames) {
      const row = document.createElement('tr');

      const nameCell = document.createElement('td');
      nameCell.className = 'col-name';
      nameCell.textContent = name;
      row.appendChild(nameCell);

      const branchCell = document.createElement('td');
      branchCell.className = 'col-branch ws-pending-text';
      branchCell.textContent = '—';
      row.appendChild(branchCell);

      const statusCell = document.createElement('td');
      const statusContainer = document.createElement('div');
      statusContainer.className = 'col-status';
      statusContainer.appendChild(createBadge('pending', 'pending'));
      statusCell.appendChild(statusContainer);
      row.appendChild(statusCell);

      tbody.appendChild(row);
    }

    return table;
  }

  private renderHeader(): HTMLElement {
    const header = document.createElement('div');
    header.className = 'page-header';

    const title = document.createElement('span');
    title.className = 'page-header-title';
    title.textContent = 'Devora';
    header.appendChild(title);

    if (this.profilesLoaded && this.profiles.length > 0) {
      header.appendChild(this.buildProfileDropdown());
    }

    return header;
  }

  private buildProfileDropdown(): HTMLElement {
    const activeProfile = this.profiles.find((p) => p.path === this.activeProfilePath);
    const items: DropdownItem[] = [
      ...this.profiles.map(
        (profile): DropdownItem => ({
          kind: 'option',
          label: profile.name,
          detail: pluralize(profile.repoCount, 'repo'),
          checked: profile.path === this.activeProfilePath,
          onSelect: () => void this.switchToProfile(profile.path),
        }),
      ),
      { kind: 'separator' },
      {
        kind: 'action',
        label: 'New Profile…',
        icon: '＋',
        onSelect: () => this.onOpenProfileManager('new'),
      },
      {
        kind: 'action',
        label: 'Manage Profiles…',
        icon: '⚙',
        onSelect: () => this.onOpenProfileManager('list'),
      },
    ];
    const handle = createDropdownMenu({
      triggerLabel: activeProfile?.name ?? 'Profiles',
      items,
    });
    handle.element.classList.add('ws-profile-dropdown');
    return handle.element;
  }

  private async switchToProfile(path: string): Promise<void> {
    if (path === this.activeProfilePath) return;
    this.activeProfilePath = path;
    this.statusCache.clear();
    this.statusErrors.clear();
    this.profilingT0 = performance.now();
    this.profilingSaved = false;
    this.profilingError = false;

    const listWorkspacesStart = performance.now();
    await this.loadWorkspaces();
    const listWorkspacesDuration = performance.now() - listWorkspacesStart;

    this.workspacesLoaded = true;

    const activeProfile = this.profiles.find((p) => p.path === this.activeProfilePath);
    this.profilingData = {
      timestamp: new Date().toISOString(),
      profileName: activeProfile?.name ?? '',
      profilePath: this.activeProfilePath ?? '',
      workspaceCount: this.workspaces.length,
      phases: {
        listProfiles: { startMs: 0, durationMs: 0 },
        listWorkspaces: {
          startMs: listWorkspacesStart - this.profilingT0,
          durationMs: listWorkspacesDuration,
        },
        render: { startMs: 0, durationMs: 0 },
        totalLoad: {
          startMs: 0,
          durationMs: performance.now() - this.profilingT0,
        },
      },
      workspaceStatuses: [],
    };

    this.render();
    this.preloadAllStatuses();
  }

  private async toggleNewForm(): Promise<void> {
    this.showNewForm = !this.showNewForm;
    if (this.showNewForm && this.activeProfilePath) {
      try {
        this.availableRepos = await invoke<RepoInfo[]>('get_registered_repos', {
          profilePath: this.activeProfilePath,
        });
      } catch (_) {
        // invoke already surfaced the error
        this.availableRepos = [];
      }
    }
    this.render();
  }

  private renderNewButton(): HTMLElement {
    const btn = document.createElement('button');
    btn.className = 'ws-new-btn';
    btn.textContent = '+ New Task';
    btn.addEventListener('click', () => this.toggleNewForm());
    return btn;
  }

  private renderNewForm(): HTMLElement {
    const form = document.createElement('div');
    form.className = 'ws-new-form';

    // Task name input
    const nameLabel = document.createElement('label');
    nameLabel.className = 'ws-new-form-label';
    nameLabel.textContent = 'Title';
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
    createBtn.addEventListener('click', () => {
      const taskName = nameInput.value.trim();
      if (!taskName || !this.activeProfilePath) return;

      const checkboxes = form.querySelectorAll<HTMLInputElement>(
        '.ws-new-form-repos input[type="checkbox"]:checked',
      );
      const repoPaths = Array.from(checkboxes).map((cb) => cb.value);

      // Hand off immediately: the controller closes the Hub, opens a tab, and streams progress.
      // Creation no longer blocks here, so the window never freezes.
      this.showNewForm = false;
      this.onStartTaskCreation(taskName, repoPaths);
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

  private renderProfilingButton(): HTMLElement | undefined {
    if (!this.profilingData) return undefined;

    const btn = document.createElement('button');
    btn.className = 'ws-profiling-btn';
    if (this.profilingSaved) {
      btn.classList.add('ws-profiling-saved');
    }
    btn.textContent = this.profilingSaved ? 'Saved!' : 'Save loading latencies';
    btn.addEventListener('click', async () => {
      if (this.profilingSaved || !this.profilingData || !this.activeProfilePath) return;
      btn.textContent = 'Saving...';
      btn.disabled = true;
      btn.classList.remove('ws-profiling-error');
      this.profilingError = false;
      try {
        const savedPath = await invoke<string>('save_profiling_report', {
          profilePath: this.activeProfilePath,
          reportJson: JSON.stringify(this.profilingData, null, 2),
        });
        this.profilingSaved = true;
        btn.textContent = `Saved! ${savedPath}`;
        btn.classList.add('ws-profiling-saved');
      } catch (_) {
        // invoke already surfaced the error
        this.profilingError = true;
        btn.textContent = 'Save failed';
        btn.classList.add('ws-profiling-error');
        btn.disabled = false;
      }
    });
    return btn;
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
        heading: 'Workspace Hub',
        keys: [
          ['j / ↓', 'Move selection down'],
          ['k / ↑', 'Move selection up'],
          ['Enter', 'Open selected workspace'],
          ['f', 'Focus filter input'],
          ['Esc', 'Unfocus filter / close cheatsheet'],
          ['1', 'Show active workspaces'],
          ['2', 'Show inactive workspaces'],
          ['3', 'Show all workspaces'],
          ['n', 'New task'],
          ['P', 'Open Profile Manager'],
          ['R', 'Refresh hub'],
          ['q', 'Close hub'],
          ['?', 'Toggle this cheatsheet'],
        ],
      },
      {
        heading: 'Global',
        keys: [
          ['Ctrl+S', 'Toggle Workspace Hub'],
          ['Shift Shift', 'Open Command Palette (double-tap)'],
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
