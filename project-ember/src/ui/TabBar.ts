import { SessionManager } from '../session/SessionManager';

export class TabBar {
  private containerEl: HTMLElement;
  private sessionManager: SessionManager;
  private paused = false;

  constructor(containerEl: HTMLElement, sessionManager: SessionManager) {
    this.containerEl = containerEl;
    this.containerEl.classList.add('tab-bar');
    this.sessionManager = sessionManager;

    // Re-render whenever sessions change
    this.sessionManager.onChange(() => this.render());
  }

  setPaused(paused: boolean): void {
    this.paused = paused;
    this.render();
  }

  render(): void {
    // Clear existing content
    this.containerEl.innerHTML = '';

    const sessions = this.sessionManager.getSessions();
    const activeId = this.sessionManager.getActiveSessionId();

    for (const session of sessions) {
      const isActive = session.id === activeId;
      const hasOverlay = session.hasSurface();

      const tabEl = document.createElement('div');
      tabEl.className = 'tab' + (isActive ? ' tab-active' : '');
      tabEl.addEventListener('click', () => {
        this.sessionManager.activateSession(session.id);
      });

      const titleEl = document.createElement('span');
      titleEl.className = 'tab-title';
      titleEl.textContent = session.title;
      tabEl.appendChild(titleEl);

      if (hasOverlay) {
        const dotEl = document.createElement('span');
        dotEl.className = 'tab-overlay-dot';
        tabEl.appendChild(dotEl);
      }

      const closeEl = document.createElement('span');
      closeEl.className = 'tab-close';
      closeEl.textContent = '×';
      closeEl.addEventListener('click', (e) => {
        e.stopPropagation();
        this.sessionManager.closeSession(session.id);
      });
      tabEl.appendChild(closeEl);

      this.containerEl.appendChild(tabEl);
    }

    // Spacer
    const spacer = document.createElement('div');
    spacer.className = 'tab-spacer';
    this.containerEl.appendChild(spacer);

    if (this.paused) {
      const hint = document.createElement('span');
      hint.className = 'global-pause-hint';
      hint.textContent = 'Paused';
      hint.title = 'Devora is globally paused. Resume Devora from the command palette.';
      this.containerEl.appendChild(hint);
    }

    // Add button
    const addEl = document.createElement('div');
    addEl.className = 'tab-add';
    addEl.textContent = '+';
    addEl.addEventListener('click', () => {
      this.sessionManager.createSession();
    });
    this.containerEl.appendChild(addEl);
  }
}
