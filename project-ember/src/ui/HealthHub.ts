/**
 * Health Hub overlay: a read-only diagnostic view of Devora's dependencies, credentials, config, and zsh completion.
 * Same lifecycle shape as WorkspaceHub / CommandPalette (getElement/load/unload).
 *
 * Data comes from the bundled `debi health --json` via the `run_health_check` Tauri command — debi stays the single source of truth for the checks.
 * DOM: `div.health-hub`.
 */

import { invoke } from '../invoke';
import { createStatusDot, StatusDotVariant } from './components/StatusDot';
import { createKeyboardHintBar } from './components/KeyboardHintBar';
import { createToast } from './components/Toast';
import { isEditableElementFocused } from './focus';
import { createVimScroll } from './vimScroll';

type HealthState = 'ok' | 'warn' | 'error' | 'info';
interface HealthItem { name: string; state: HealthState; message: string; version: string | null; path: string | null; fixHint: string | null }
interface HealthSection { title: string; count: string | null; state: HealthState; items: HealthItem[] }
interface HealthReport {
  version: string;
  overallState: HealthState;
  overallLabel: string;
  tiles: { label: string; met: number; total: number; state: HealthState }[];
  sections: HealthSection[];
}
const STATE_TO_DOT: Record<HealthState, StatusDotVariant> = {
  ok: 'clean', warn: 'modified', error: 'error', info: 'pending',
};

export interface HealthHubOptions {
  /** Resolves the active profile path passed to the backend (for the tracker-credential row). */
  getProfilePath: () => string | null | undefined;
}

export class HealthHub {
  private getProfilePath: () => string | null | undefined;
  private containerEl: HTMLElement;
  private report: HealthReport | null = null;
  private loading = false;
  private error: string | null = null;
  private loadToken = 0;
  private scroll = createVimScroll();

  constructor(options: HealthHubOptions) {
    this.getProfilePath = options.getProfilePath;
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'health-hub';
  }

  getElement(): HTMLElement {
    return this.containerEl;
  }

  load(): void {
    this.scroll.reset();
    void this.fetchReport();
  }

  unload(): void {
    this.loadToken++;
    this.report = null;
    this.error = null;
    this.loading = false;
    this.containerEl.innerHTML = '';
  }

  private async fetchReport(): Promise<void> {
    this.loading = true;
    this.error = null;
    this.render();

    const token = ++this.loadToken;
    try {
      const report = await invoke<HealthReport>('run_health_check', {
        profilePath: this.getProfilePath() ?? null,
      });
      if (token !== this.loadToken) return;
      this.report = report;
      this.error = null;
    } catch (e) {
      if (token !== this.loadToken) return;
      // invoke already surfaced a banner for command failures; this is the in-body fallback (covers both command rejection and JSON parse errors).
      this.report = null;
      this.error = `Could not run the health check: ${String(e)}`;
    } finally {
      if (token === this.loadToken) {
        this.loading = false;
        this.render();
      }
    }
  }

  /** The page layer's key handler (routed by the LayerStack). Returns `true` when it consumes the key; q/Esc dismissal flows through the layer's default onUserDismissRequest. */
  handleKey(e: KeyboardEvent): boolean {
    if (isEditableElementFocused()) return false;
    if (e.key === 'r' && !this.loading) {
      void this.fetchReport();
      return true;
    }
    const content = this.containerEl.querySelector<HTMLElement>('.health-content');
    if (content && this.scroll.handleKey(e, content)) return true;
    return false;
  }

  // --- Rendering ---

  private render(): void {
    this.containerEl.innerHTML = '';
    this.containerEl.appendChild(this.renderHeader());

    const accent = document.createElement('div');
    accent.className = 'panel-accent';
    this.containerEl.appendChild(accent);

    const content = document.createElement('div');
    content.className = 'health-content';

    if (this.loading) {
      const loading = document.createElement('div');
      loading.className = 'health-loading';
      loading.textContent = 'Running health check…';
      content.appendChild(loading);
    } else if (this.error) {
      const err = document.createElement('div');
      err.className = 'health-error';
      err.textContent = this.error;
      content.appendChild(err);
    } else if (this.report) {
      content.appendChild(this.renderTiles(this.report));
      for (const section of this.report.sections) content.appendChild(this.renderSection(section));
    }

    this.containerEl.appendChild(content);

    this.containerEl.appendChild(
      createKeyboardHintBar({
        hints: [
          { keys: 'j/k · Ctrl+D/U · gg/G', description: 'scroll' },
          { keys: 'r', description: 're-run' },
          { keys: 'q/Esc', description: 'close' },
        ],
      }),
    );
  }

  private renderHeader(): HTMLElement {
    const header = document.createElement('div');
    header.className = 'page-header';

    const title = document.createElement('span');
    title.className = 'page-header-title';
    title.textContent = 'Health Hub';
    header.appendChild(title);

    const meta = document.createElement('div');
    meta.className = 'health-header-meta';

    if (this.report) {
      const version = document.createElement('span');
      version.className = 'health-version-pill';
      version.textContent = /^\d/.test(this.report.version)
        ? `v${this.report.version}`
        : this.report.version;
      meta.appendChild(version);

      meta.appendChild(this.renderOverallPill(this.report));
    }

    const rerun = document.createElement('button');
    rerun.className = 'health-rerun-btn';
    rerun.textContent = '↻ Re-run';
    rerun.disabled = this.loading;
    rerun.addEventListener('click', () => void this.fetchReport());
    meta.appendChild(rerun);

    header.appendChild(meta);
    return header;
  }

  private renderOverallPill(r: HealthReport): HTMLElement {
    const { overallState: state, overallLabel: label } = r;
    const pill = document.createElement('span');
    pill.className = `health-overall-pill ${state}`;
    pill.appendChild(this.dot(state));
    const text = document.createElement('span');
    text.textContent = label;
    pill.appendChild(text);
    return pill;
  }

  private renderTiles(r: HealthReport): HTMLElement {
    const tiles = document.createElement('div');
    tiles.className = 'health-tiles';
    for (const tile of r.tiles) tiles.appendChild(this.renderTile(tile.label, tile.met, tile.total, tile.state));
    return tiles;
  }

  private renderTile(label: string, met: number, total: number, state: HealthState): HTMLElement {

    const tile = document.createElement('div');
    tile.className = 'health-tile';

    const labelEl = document.createElement('div');
    labelEl.className = 'health-tile-label';
    labelEl.textContent = label;
    tile.appendChild(labelEl);

    const valueEl = document.createElement('div');
    valueEl.className = `health-tile-value ${state}`;
    valueEl.textContent = `${met}/${total}`;
    tile.appendChild(valueEl);

    const meter = document.createElement('div');
    meter.className = 'health-tile-meter';
    const fill = document.createElement('span');
    fill.className = `health-tile-meter-fill ${state}`;
    fill.style.width = total > 0 ? `${Math.round((met / total) * 100)}%` : '0%';
    meter.appendChild(fill);
    tile.appendChild(meter);

    return tile;
  }

  private renderSection(section: HealthSection): HTMLElement {
    const shell = this.sectionShell(section.title, section.count ?? '', section.state, section.count !== null);
    for (const item of section.items) {
      const row = this.rowShell(item.state, item.name);
      if (item.version) this.appendText(row, 'health-version-badge', item.version);
      if (item.message) this.appendText(row, 'health-row-msg', item.message);
      if (item.path) this.appendText(row, 'health-row-path', item.path);
      if (item.fixHint) row.appendChild(this.renderFixHint(item.fixHint));
      shell.card.appendChild(row);
    }
    return shell.section;
  }

  private renderFixHint(command: string): HTMLElement {
    const wrap = document.createElement('span');
    wrap.className = 'health-fixhint';

    const code = document.createElement('code');
    code.className = 'health-fixhint-cmd';
    code.textContent = command;
    wrap.appendChild(code);

    const copy = document.createElement('button');
    copy.className = 'health-copy-btn';
    copy.title = 'Copy to clipboard';
    copy.setAttribute('aria-label', `Copy: ${command}`);
    copy.textContent = '⧉';
    copy.addEventListener('click', () => void this.copyToClipboard(command));
    wrap.appendChild(copy);

    return wrap;
  }

  private async copyToClipboard(text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      const toast = createToast('Copied to clipboard');
      setTimeout(() => void toast.dismiss(), 1500);
    } catch (_) {
      const toast = createToast('Copy failed');
      setTimeout(() => void toast.dismiss(), 1500);
    }
  }

  // --- Small builders ---

  private dot(state: HealthState): HTMLElement {
    return createStatusDot(STATE_TO_DOT[state]);
  }

  /** Append a `<span class={className}>{text}</span>` to `parent` and return it. */
  private appendText(parent: HTMLElement, className: string, text: string): HTMLElement {
    const span = document.createElement('span');
    span.className = className;
    span.textContent = text;
    parent.appendChild(span);
    return span;
  }

  private sectionShell(
    title: string,
    count: string,
    countState: HealthState,
    showCount: boolean,
  ): { section: HTMLElement; card: HTMLElement } {
    const section = document.createElement('div');
    section.className = 'health-section';

    const head = document.createElement('div');
    head.className = 'health-section-head';
    this.appendText(head, 'health-section-title', title);
    if (showCount) {
      this.appendText(head, `health-section-count ${countState}`, count);
    }
    section.appendChild(head);

    const card = document.createElement('div');
    card.className = 'health-rows';
    section.appendChild(card);

    return { section, card };
  }

  private rowShell(state: HealthState, name: string): HTMLElement {
    const row = document.createElement('div');
    row.className = 'health-row';
    row.appendChild(this.dot(state));
    this.appendText(row, 'health-row-name', name);
    return row;
  }
}
