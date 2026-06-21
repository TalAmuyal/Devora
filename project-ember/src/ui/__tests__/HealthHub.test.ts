import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { HealthHub } from '../HealthHub';
import { invoke } from '../../invoke';

vi.mock('../../invoke', () => ({ invoke: vi.fn() }));
const invokeMock = vi.mocked(invoke);

const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));

interface ReportShape {
  version: string;
  config: { path: string; found: boolean; fixHint?: string };
  completion: { path: string; found: boolean; fixHint?: string };
  required: Array<{ name: string; found: boolean; version: string; path: string }>;
  optional: Array<{ name: string; found: boolean; version: string; path: string }>;
  credentials: Array<{ name: string; status: string; message: string; fixHint?: string }>;
  summary: {
    requiredMet: number;
    requiredTotal: number;
    optionalMet: number;
    optionalTotal: number;
    credentialsMet: number;
    credentialsTotal: number;
  };
}

function baseReport(): ReportShape {
  return {
    version: '0.42.0',
    config: { path: '/home/u/.config/devora/config.json', found: true },
    completion: {
      path: '/home/u/.zsh/completions/_debi',
      found: false,
      fixHint: 'debi completion zsh > ~/.zsh/completions/_debi',
    },
    required: [
      { name: 'claude', found: true, version: '2.1.4', path: '/u/bin/claude' },
      { name: 'git', found: true, version: '2.45', path: '/usr/bin/git' },
      { name: 'uv', found: true, version: '0.5', path: '/u/bin/uv' },
      { name: 'zsh', found: true, version: '5.9', path: '/bin/zsh' },
    ],
    optional: [
      { name: 'nvim', found: true, version: '0.10', path: '/u/bin/nvim' },
      { name: 'mise', found: true, version: '2024', path: '/u/bin/mise' },
      { name: 'gh', found: false, version: '', path: '' },
    ],
    credentials: [
      { name: 'GitHub', status: 'unchecked', message: 'gh not detected' },
      { name: 'task-tracker', status: 'info', message: 'not configured (optional)' },
    ],
    summary: {
      requiredMet: 4,
      requiredTotal: 4,
      optionalMet: 2,
      optionalTotal: 3,
      credentialsMet: 0,
      credentialsTotal: 1,
    },
  };
}

function resolveWith(report: ReportShape): void {
  invokeMock.mockResolvedValue(JSON.stringify(report));
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
    expect(Array.from(tiles).map((t) => t.textContent)).toEqual(['4/4', '2/3', '0/1', '1/2']);
  });

  it('shows "Healthy" only when nothing is missing', async () => {
    const report = baseReport();
    report.optional = report.optional.map((d) => ({ ...d, found: true, version: '1', path: '/x' }));
    report.completion = { path: '/c', found: true };
    report.credentials = [{ name: 'GitHub', status: 'ok', message: 'Logged in' }];
    report.summary = {
      requiredMet: 4,
      requiredTotal: 4,
      optionalMet: 3,
      optionalTotal: 3,
      credentialsMet: 1,
      credentialsTotal: 1,
    };
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
    report.required[2] = { name: 'uv', found: false, version: '', path: '' };
    report.summary.requiredMet = 3;
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
    report.credentials = [
      {
        name: 'GitHub',
        status: 'failed',
        message: 'no token stored (run: gh auth login)',
        fixHint: 'gh auth login',
      },
    ];
    report.summary.credentialsMet = 0;
    report.summary.credentialsTotal = 1;
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

  it('shows a parse-error state when the JSON is malformed', async () => {
    invokeMock.mockResolvedValue('not json{');
    const el = await mount();
    expect(el.querySelector('.health-error')?.textContent).toContain('Could not parse');
  });

  it('re-runs the check when "r" is pressed', async () => {
    await mount();
    expect(invokeMock).toHaveBeenCalledTimes(1);
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', bubbles: true, cancelable: true }));
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('re-runs the check when the Re-run button is clicked', async () => {
    const el = await mount();
    el.querySelector<HTMLButtonElement>('.health-rerun-btn')!.click();
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('stops responding to "r" after unload', async () => {
    await mount();
    hub.unload();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'r', bubbles: true, cancelable: true }));
    await flush();
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});
