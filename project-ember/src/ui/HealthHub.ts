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

interface DependencyStatus {
  name: string;
  found: boolean;
  version: string;
  path: string;
}

type CredentialStatus = 'ok' | 'failed' | 'unchecked' | 'info';

interface CredentialReport {
  name: string;
  status: CredentialStatus | string;
  message: string;
  fixHint?: string;
}

interface FileCheck {
  path: string;
  found: boolean;
  fixHint?: string;
}

interface HealthSummary {
  requiredMet: number;
  requiredTotal: number;
  optionalMet: number;
  optionalTotal: number;
  credentialsMet: number;
  credentialsTotal: number;
}

interface HealthReport {
  version: string;
  config: FileCheck;
  completion: FileCheck;
  required: DependencyStatus[];
  optional: DependencyStatus[];
  credentials: CredentialReport[];
  summary: HealthSummary;
}

/** Health-domain status, mapped onto the shared StatusDot variants. */
type HealthState = 'ok' | 'warn' | 'error' | 'info';

const STATE_TO_DOT: Record<HealthState, StatusDotVariant> = {
  ok: 'clean',
  warn: 'modified',
  error: 'error',
  info: 'pending',
};

const CRED_STATE: Record<string, HealthState> = {
  ok: 'ok',
  failed: 'error',
  unchecked: 'warn',
  info: 'info',
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

  private keyHandler = (e: KeyboardEvent) => this.handleKeyDown(e);

  constructor(options: HealthHubOptions) {
    this.getProfilePath = options.getProfilePath;
    this.containerEl = document.createElement('div');
    this.containerEl.className = 'health-hub';
  }

  getElement(): HTMLElement {
    return this.containerEl;
  }

  load(): void {
    window.addEventListener('keydown', this.keyHandler, true);
    void this.fetchReport();
  }

  unload(): void {
    window.removeEventListener('keydown', this.keyHandler, true);
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
      const json = await invoke<string>('run_health_check', {
        profilePath: this.getProfilePath() ?? null,
      });
      if (token !== this.loadToken) return;
      this.report = JSON.parse(json) as HealthReport;
      this.error = null;
    } catch (e) {
      if (token !== this.loadToken) return;
      // invoke already surfaced a banner for command failures; this is the in-body fallback (covers both command rejection and JSON parse errors).
      this.report = null;
      this.error = e instanceof SyntaxError ? 'Could not parse the health report.' : 'Could not run the health check.';
    } finally {
      if (token === this.loadToken) {
        this.loading = false;
        this.render();
      }
    }
  }

  private handleKeyDown(e: KeyboardEvent): void {
    if (isEditableElementFocused()) return;
    // q/Esc dismissal is handled globally (KeyboardShortcuts → dismissActiveOverlay).
    if (e.key === 'r' && !this.loading) {
      e.preventDefault();
      e.stopPropagation();
      void this.fetchReport();
    }
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
      content.appendChild(this.renderDepSection('Required dependencies', this.report.required, true));
      content.appendChild(this.renderDepSection('Optional dependencies', this.report.optional, false));
      content.appendChild(this.renderCredentialSection(this.report));
      content.appendChild(this.renderConfigSection(this.report));
    }

    this.containerEl.appendChild(content);

    this.containerEl.appendChild(
      createKeyboardHintBar({
        hints: [
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

  /** Overall verdict: missing required → error; any optional/credential/completion gap → warn; else ok. */
  private overallState(r: HealthReport): { state: HealthState; label: string } {
    if (r.summary.requiredMet < r.summary.requiredTotal) {
      return { state: 'error', label: 'Missing required' };
    }
    const hasGap =
      r.summary.optionalMet < r.summary.optionalTotal ||
      r.summary.credentialsMet < r.summary.credentialsTotal ||
      !r.completion.found;
    if (hasGap) {
      return { state: 'warn', label: 'Issues found' };
    }
    return { state: 'ok', label: 'Healthy' };
  }

  private renderOverallPill(r: HealthReport): HTMLElement {
    const { state, label } = this.overallState(r);
    const pill = document.createElement('span');
    pill.className = `health-overall-pill ${state}`;
    pill.appendChild(this.dot(state));
    const text = document.createElement('span');
    text.textContent = label;
    pill.appendChild(text);
    return pill;
  }

  private renderTiles(r: HealthReport): HTMLElement {
    const setupMet = (r.config.found ? 1 : 0) + (r.completion.found ? 1 : 0);

    const tiles = document.createElement('div');
    tiles.className = 'health-tiles';
    tiles.appendChild(
      this.renderTile('Required', r.summary.requiredMet, r.summary.requiredTotal, true),
    );
    tiles.appendChild(
      this.renderTile('Optional', r.summary.optionalMet, r.summary.optionalTotal, false),
    );
    tiles.appendChild(
      this.renderTile('Credentials', r.summary.credentialsMet, r.summary.credentialsTotal, false),
    );
    tiles.appendChild(this.renderTile('Setup', setupMet, 2, false));
    return tiles;
  }

  private renderTile(label: string, met: number, total: number, requiredGroup: boolean): HTMLElement {
    const complete = met >= total;
    const state: HealthState = complete ? 'ok' : requiredGroup ? 'error' : 'warn';

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

  private renderDepSection(
    title: string,
    deps: DependencyStatus[],
    requiredGroup: boolean,
  ): HTMLElement {
    const met = deps.filter((d) => d.found).length;
    const countState: HealthState = met >= deps.length ? 'ok' : requiredGroup ? 'error' : 'warn';
    const section = this.sectionShell(title, `${met} / ${deps.length}`, countState, true);
    const card = section.card;

    for (const dep of deps) {
      const state: HealthState = dep.found ? 'ok' : requiredGroup ? 'error' : 'warn';
      const row = this.rowShell(state, dep.name);

      if (dep.found) {
        this.appendText(row, 'health-version-badge', dep.version || '—');
        this.appendText(row, 'health-row-path', dep.path);
      } else {
        this.appendText(row, 'health-row-msg', 'not found');
      }

      card.appendChild(row);
    }

    return section.section;
  }

  private renderCredentialSection(r: HealthReport): HTMLElement {
    const countState: HealthState =
      r.summary.credentialsMet >= r.summary.credentialsTotal ? 'ok' : 'warn';
    const section = this.sectionShell(
      'Credentials',
      `${r.summary.credentialsMet} / ${r.summary.credentialsTotal}`,
      countState,
      true,
    );

    for (const cred of r.credentials) {
      const state = CRED_STATE[cred.status] ?? 'info';
      const row = this.rowShell(state, cred.name);

      this.appendText(row, 'health-row-msg', cred.message);

      if (cred.fixHint) {
        row.appendChild(this.renderFixHint(cred.fixHint));
      }

      section.card.appendChild(row);
    }

    return section.section;
  }

  private renderConfigSection(r: HealthReport): HTMLElement {
    const section = this.sectionShell('Configuration', '', 'ok', false);

    // Config file (a missing config is informational, like the OG "(not found)").
    const configRow = this.rowShell(r.config.found ? 'ok' : 'warn', 'config');
    this.appendText(configRow, 'health-row-msg', r.config.found ? 'found' : 'not found');
    this.appendText(configRow, 'health-row-path', r.config.path);
    section.card.appendChild(configRow);

    // zsh completion.
    const compRow = this.rowShell(r.completion.found ? 'ok' : 'warn', 'zsh completion');
    this.appendText(compRow, 'health-row-msg', r.completion.found ? 'installed' : 'not installed');
    if (r.completion.found) {
      this.appendText(compRow, 'health-row-path', r.completion.path);
    } else if (r.completion.fixHint) {
      compRow.appendChild(this.renderFixHint(r.completion.fixHint));
    }
    section.card.appendChild(compRow);

    return section.section;
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
