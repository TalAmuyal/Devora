import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { HealthHub } from '../HealthHub';
import { invoke } from '../../invoke';

vi.mock('../../invoke', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

type HealthState = 'ok' | 'warn' | 'error' | 'info';
interface ReportShape {
  version: string;
  overallState: HealthState;
  overallLabel: string;
  tiles: { label: string; met: number; total: number; state: HealthState }[];
  sections: { title: string; count: string | null; state: HealthState; items: { name: string; state: HealthState; message: string; version: string | null; path: string | null; fixHint: string | null }[] }[];
}

function baseReport(): ReportShape {
  return {
    version: '0.42.0', overallState: 'warn', overallLabel: 'Issues found',
    tiles: [
      { label: 'Required', met: 3, total: 3, state: 'ok' },
      { label: 'Optional', met: 2, total: 3, state: 'warn' },
      { label: 'Credentials', met: 0, total: 1, state: 'warn' },
      { label: 'Setup', met: 1, total: 2, state: 'warn' },
    ],
    sections: [
      { title: 'Required dependencies', count: '3 / 3', state: 'ok', items: [] },
      { title: 'Optional dependencies', count: '2 / 3', state: 'warn', items: [
        { name: 'gh', state: 'warn', message: 'not found', version: null, path: null, fixHint: null },
      ] },
      { title: 'Credentials', count: '0 / 1', state: 'warn', items: [] },
      { title: 'Configuration', count: null, state: 'ok', items: [
        { name: 'zsh completion', state: 'warn', message: 'not installed', version: null, path: null, fixHint: 'debi completion zsh > ~/.zsh/completions/_debi' },
      ] },
    ],
  };
}

function resolveWith(report: ReportShape): void {
  invokeMock.mockResolvedValue(report);
}

let hub: HealthHub;
const writeText = vi.fn();

beforeEach(() => {
  invokeMock.mockReset();
  resolveWith(baseReport());
  writeText.mockReset().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
  });
});

afterEach(() => {
  hub?.unload();
  document.body.innerHTML = '';
});

async function mount(profilePath: string | null = '/profile'): Promise<HTMLElement> {
  hub = new HealthHub({ getProfilePath: () => profilePath });
  document.body.appendChild(hub.getElement());
  hub.load();
  await flush();
  return hub.getElement();
}

function text(el: HTMLElement): string {
  return el.textContent ?? '';
}

describe('HealthHub', () => {
  it('shows a loading state before the report resolves', async () => {
    hub = new HealthHub({ getProfilePath: () => '/profile' });
    document.body.appendChild(hub.getElement());
    hub.load();
    // Before flushing the invoke promise, the loading state is visible.
    expect(text(hub.getElement())).toContain('Running health check');
    await flush();
  });

  it('passes the active profile path to run_health_check', async () => {
    await mount('/my/profile');
    expect(invokeMock).toHaveBeenCalledWith('run_health_check', { profilePath: '/my/profile' });
  });

  it('renders all four sections after the report loads', async () => {
    const el = await mount();
    const body = text(el);
    expect(body).toContain('Required dependencies');
    expect(body).toContain('Optional dependencies');
    expect(body).toContain('Credentials');
    expect(body).toContain('Configuration');
  });

  it('renders the version pill and summary tiles', async () => {
    const el = await mount();
    expect(text(el)).toContain('v0.42.0');
    const tiles = el.querySelectorAll('.health-tile-value');
    expect(Array.from(tiles).map((t) => t.textContent)).toEqual(['3/3', '2/3', '0/1', '1/2']);
  });

  it('shows "Healthy" only when nothing is missing', async () => {
    const report = baseReport();
    report.overallState = 'ok';
    report.overallLabel = 'Healthy';
    resolveWith(report);
    const el = await mount();
    const pill = el.querySelector('.health-overall-pill')!;
    expect(pill.classList.contains('ok')).toBe(true);
    expect(pill.textContent).toContain('Healthy');
  });

  it('shows "Issues found" when an optional/credential/completion gap exists', async () => {
    const el = await mount();
    const pill = el.querySelector('.health-overall-pill')!;
    expect(pill.classList.contains('warn')).toBe(true);
    expect(pill.textContent).toContain('Issues found');
  });

  it('shows "Missing required" when a required dependency is missing', async () => {
    const report = baseReport();
    report.overallState = 'error';
    report.overallLabel = 'Missing required';
    resolveWith(report);
    const el = await mount();
    const pill = el.querySelector('.health-overall-pill')!;
    expect(pill.classList.contains('error')).toBe(true);
    expect(pill.textContent).toContain('Missing required');
  });

  it('renders a missing optional dependency as "not found"', async () => {
    const el = await mount();
    const body = text(el);
    expect(body).toContain('gh');
    expect(body).toContain('not found');
  });

  it('renders the completion fix hint with a copy button when missing', async () => {
    const el = await mount();
    const fix = el.querySelector('.health-fixhint-cmd');
    expect(fix?.textContent).toBe('debi completion zsh > ~/.zsh/completions/_debi');
  });

  it('copies a fix hint to the clipboard when the copy button is clicked', async () => {
    const report = baseReport();
    report.sections[2].items = [
      { name: 'GitHub', state: 'error', message: 'no token stored', version: null, path: null, fixHint: 'gh auth login' },
    ];
    resolveWith(report);
    const el = await mount();

    // Two copy buttons: GitHub credential + zsh completion. Click the first.
    const copyBtn = el.querySelector<HTMLButtonElement>('.health-copy-btn')!;
    copyBtn.click();
    await flush();

    expect(writeText).toHaveBeenCalledWith('gh auth login');
    expect(document.querySelector('.toast')?.textContent).toContain('Copied');
  });

  it('shows an error state when the command rejects', async () => {
    invokeMock.mockRejectedValue(new Error('boom'));
    const el = await mount();
    expect(el.querySelector('.health-error')?.textContent).toContain('Could not run the health check');
  });

  it('shows the executable detail when the report cannot be parsed', async () => {
    invokeMock.mockRejectedValue(new Error('/fixture/debi output did not match'));
    const el = await mount();
    expect(el.querySelector('.health-error')?.textContent).toContain('/fixture/debi output did not match');
  });

  it('re-runs the check when "r" is pressed, consuming the key', async () => {
    await mount();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    const consumed = hub.handleKey(new KeyboardEvent('keydown', { key: 'r' }));
    await flush();
    expect(consumed).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('re-runs the check when the Re-run button is clicked', async () => {
    const el = await mount();
    el.querySelector<HTMLButtonElement>('.health-rerun-btn')!.click();
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('leaves unrelated keys for the layer stack (returns false, no re-run)', async () => {
    await mount();
    expect(hub.handleKey(new KeyboardEvent('keydown', { key: 'q' }))).toBe(false);
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
