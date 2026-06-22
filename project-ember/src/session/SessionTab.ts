import { TerminalPane } from '../terminal/TerminalPane';
import { createPreviewPane, PreviewPaneHandle } from '../ui/components/PreviewPane';
import { createPreviewDivider, PreviewDividerHandle } from '../ui/components/PreviewDivider';

interface PreviewEntry {
  handle: PreviewPaneHandle;
  divider: PreviewDividerHandle;
}

export class SessionTab {
  readonly id: number;
  title: string;
  // null for sessions not tied to a workspace (plain shells) or while a task is still being created; set once via setWorkspacePath when the workspace becomes available.
  private _workspacePath: string | null;
  readonly profilePath: string | null; // profile the workspace belongs to; null for plain shells
  readonly containerEl: HTMLElement;
  readonly terminalPane: TerminalPane;
  panelOverlayEl: HTMLElement | null = null; // for panel overlay tied to this tab

  // The terminal and any preview panes share a horizontal flex row inside containerEl.
  private readonly splitEl: HTMLElement;
  private readonly terminalWrapperEl: HTMLElement;
  private previews: PreviewEntry[] = [];

  private onTitleChangeCallback?: (id: number, title: string) => void;

  constructor(
    id: number,
    title: string,
    workspacePath: string | null,
    profilePath: string | null = null,
  ) {
    this.id = id;
    this.title = title;
    this._workspacePath = workspacePath;
    this.profilePath = profilePath;

    // Create a container div for this session's content
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'session-content';
    this.containerEl.style.display = 'none'; // hidden until activated
    this.containerEl.style.width = '100%';
    this.containerEl.style.height = '100%';

    // Horizontal split: terminal (elastic) on the left, preview panes to the right.
    this.splitEl = document.createElement('div');
    this.splitEl.className = 'session-split';
    this.containerEl.appendChild(this.splitEl);

    this.terminalWrapperEl = document.createElement('div');
    this.terminalWrapperEl.className = 'terminal-pane-wrapper';
    this.splitEl.appendChild(this.terminalWrapperEl);

    this.terminalPane = new TerminalPane(this.terminalWrapperEl);
    // Test hook: terminal-helper.ts and session.steps.ts read the session's xterm instance via containerEl.
    // The terminal mounts into terminalWrapperEl, so re-expose the handle on containerEl to keep that access point stable.
    (this.containerEl as { __xtermTerminal?: unknown }).__xtermTerminal =
      (this.terminalWrapperEl as { __xtermTerminal?: unknown }).__xtermTerminal;
    this.terminalPane.onTitleChange((newTitle) => {
      this.title = newTitle;
      this.onTitleChangeCallback?.(this.id, newTitle);
    });
  }

  get workspacePath(): string | null {
    return this._workspacePath;
  }

  /** Set the workspace once it becomes available (e.g. after a pending task creation completes). */
  setWorkspacePath(workspacePath: string): void {
    this._workspacePath = workspacePath;
  }

  onTitleChange(callback: (id: number, title: string) => void): void {
    this.onTitleChangeCallback = callback;
  }

  /** Programmatic rename; fires the same pipeline as terminal-emitted title changes. */
  setTitle(title: string): void {
    this.title = title;
    this.onTitleChangeCallback?.(this.id, title);
  }

  onExit(callback: () => void): void {
    this.terminalPane.onExit(() => callback());
  }

  getPtyId(): number | null {
    return this.terminalPane.getPtyId();
  }

  async connect(cwd?: string, appCommand?: string): Promise<void> {
    await this.terminalPane.connect(cwd, appCommand, this.profilePath ?? undefined);
  }

  /**
   * Open (or refresh) a rendered preview of a file beside the terminal.
   * Re-previewing an already-open path refreshes its pane; otherwise the file replaces the current preview, or — with `stack` — opens an additional pane.
   */
  openPreview(path: string, stack: boolean): void {
    const existing = this.previews.find((p) => p.handle.path === path);
    if (existing) {
      existing.handle.refresh();
      return;
    }
    if (!stack) {
      this.closeAllPreviews();
    }
    this.addPreview(path);
  }

  getPreviewCount(): number {
    return this.previews.length;
  }

  private addPreview(path: string): void {
    const handle = createPreviewPane({
      path,
      onClose: () => this.closePreview(path),
    });
    const divider = createPreviewDivider({
      paneEl: handle.el,
      terminalEl: this.terminalWrapperEl,
      splitEl: this.splitEl,
    });

    this.previews.push({ handle, divider });
    this.splitEl.appendChild(divider.el);
    this.splitEl.appendChild(handle.el);

    // The terminal just lost width; snap it (and the PTY) to the new size.
    this.terminalPane.fit();
  }

  private closePreview(path: string): void {
    const index = this.previews.findIndex((p) => p.handle.path === path);
    if (index === -1) return;
    this.removeEntry(this.previews[index]);
    this.previews.splice(index, 1);
    // The terminal just regained width; resize it and return focus (the user clicked ×).
    this.terminalPane.fit();
    this.terminalPane.focus();
  }

  private closeAllPreviews(): void {
    for (const entry of this.previews) {
      this.removeEntry(entry);
    }
    this.previews = [];
  }

  private removeEntry(entry: PreviewEntry): void {
    entry.divider.dispose();
    entry.divider.el.remove();
    entry.handle.el.remove();
  }

  show(): void {
    this.containerEl.style.display = 'block';
    // Now that the pane has a layout box again, snap it to the current size — covers a window resize that happened while this tab was hidden.
    this.terminalPane.fit();
    this.terminalPane.focus();
  }

  hide(): void {
    this.containerEl.style.display = 'none';
  }

  dispose(): void {
    this.closeAllPreviews();
    this.terminalPane.dispose();
    this.containerEl.remove();
  }
}
