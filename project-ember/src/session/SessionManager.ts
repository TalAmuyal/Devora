import { SessionTab } from './SessionTab';

export class SessionManager {
  private sessions: SessionTab[] = [];
  private activeSessionId: number | null = null;
  private nextId = 1;
  private mainPanelEl: HTMLElement;

  // Callback for external UI (tab bar) to listen to
  private onChangeCallback?: () => void;

  constructor(mainPanelEl: HTMLElement) {
    this.mainPanelEl = mainPanelEl;
  }

  onChange(callback: () => void): void {
    this.onChangeCallback = callback;
  }

  async createSession(
    title: string = 'Shell',
    cwd?: string,
    appCommand?: string,
    workspacePath: string | null = null,
    profilePath: string | null = null,
  ): Promise<SessionTab> {
    const id = this.nextId++;
    const session = new SessionTab(id, title, workspacePath, profilePath);

    session.onTitleChange((_id, _title) => {
      this.onChangeCallback?.();
    });

    session.onExit(() => {
      this.closeSession(id);
    });

    this.mainPanelEl.appendChild(session.containerEl);
    this.sessions.push(session);

    this.activateSession(id);
    await session.connect(cwd, appCommand);

    this.onChangeCallback?.();
    return session;
  }

  closeSession(id: number): void {
    const index = this.sessions.findIndex((s) => s.id === id);
    if (index === -1) return;

    const session = this.sessions[index];
    const wasActive = this.activeSessionId === id;

    session.dispose();
    this.sessions.splice(index, 1);

    if (wasActive && this.sessions.length > 0) {
      // Activate the nearest session
      const newIndex = Math.min(index, this.sessions.length - 1);
      this.activateSession(this.sessions[newIndex].id);
    } else if (this.sessions.length === 0) {
      this.activeSessionId = null;
    }

    this.onChangeCallback?.();
  }

  activateSession(id: number): void {
    for (const session of this.sessions) {
      if (session.id === id) {
        session.show();
        this.activeSessionId = id;
      } else {
        session.hide();
      }
    }
    this.onChangeCallback?.();
  }

  getActiveSession(): SessionTab | null {
    return this.sessions.find((s) => s.id === this.activeSessionId) ?? null;
  }

  getSessions(): ReadonlyArray<SessionTab> {
    return this.sessions;
  }

  getActiveSessionId(): number | null {
    return this.activeSessionId;
  }

  activatePrevious(): void {
    if (this.sessions.length <= 1) return;
    const index = this.sessions.findIndex((s) => s.id === this.activeSessionId);
    if (index <= 0) {
      this.activateSession(this.sessions[this.sessions.length - 1].id);
    } else {
      this.activateSession(this.sessions[index - 1].id);
    }
  }

  activateNext(): void {
    if (this.sessions.length <= 1) return;
    const index = this.sessions.findIndex((s) => s.id === this.activeSessionId);
    if (index >= this.sessions.length - 1) {
      this.activateSession(this.sessions[0].id);
    } else {
      this.activateSession(this.sessions[index + 1].id);
    }
  }

  moveTabBackward(): void {
    const index = this.sessions.findIndex((s) => s.id === this.activeSessionId);
    if (index <= 0) return;
    [this.sessions[index - 1], this.sessions[index]] = [
      this.sessions[index],
      this.sessions[index - 1],
    ];
    this.onChangeCallback?.();
  }

  moveTabForward(): void {
    const index = this.sessions.findIndex((s) => s.id === this.activeSessionId);
    if (index === -1 || index >= this.sessions.length - 1) return;
    [this.sessions[index], this.sessions[index + 1]] = [
      this.sessions[index + 1],
      this.sessions[index],
    ];
    this.onChangeCallback?.();
  }

  getSessionCount(): number {
    return this.sessions.length;
  }
}
