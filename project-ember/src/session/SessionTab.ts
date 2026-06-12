import { TerminalPane } from '../terminal/TerminalPane';

export class SessionTab {
  readonly id: number;
  title: string;
  readonly workspacePath: string | null; // null for sessions not tied to a workspace (plain shells)
  readonly containerEl: HTMLElement;
  readonly terminalPane: TerminalPane;
  panelOverlayEl: HTMLElement | null = null; // for panel overlay tied to this tab

  private onTitleChangeCallback?: (id: number, title: string) => void;

  constructor(id: number, title: string, workspacePath: string | null) {
    this.id = id;
    this.title = title;
    this.workspacePath = workspacePath;

    // Create a container div for this session's content
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'session-content';
    this.containerEl.style.display = 'none'; // hidden until activated
    this.containerEl.style.width = '100%';
    this.containerEl.style.height = '100%';

    this.terminalPane = new TerminalPane(this.containerEl);
    this.terminalPane.onTitleChange((newTitle) => {
      this.title = newTitle;
      this.onTitleChangeCallback?.(this.id, newTitle);
    });
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
    await this.terminalPane.connect(cwd, appCommand);
  }

  show(): void {
    this.containerEl.style.display = 'block';
    this.terminalPane.focus();
  }

  hide(): void {
    this.containerEl.style.display = 'none';
  }

  dispose(): void {
    this.terminalPane.dispose();
    this.containerEl.remove();
  }
}
