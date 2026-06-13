import { describe, it, expect, vi, afterEach } from 'vitest';
import { ProfileManager, ProfileManagerCallbacks } from '../ProfileManager';
import { invoke } from '../../invoke';

vi.mock('../../invoke', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

const PROFILES = [
  { name: 'Work', path: '/profiles/work', repoCount: 2 },
  { name: 'Personal', path: '/profiles/personal', repoCount: 1 },
];

function mockBackend(overrides?: {
  profiles?: typeof PROFILES;
  onUnregister?: (path: string) => void;
}): void {
  let profiles = overrides?.profiles ?? PROFILES;
  invokeMock.mockImplementation(async (cmd: string, args?: unknown) => {
    switch (cmd) {
      case 'list_profiles':
        return profiles;
      case 'get_registered_repos':
        return [
          { name: 'repo-a', path: '/repos/repo-a', source: 'auto-discovered' },
          { name: 'repo-b', path: '/elsewhere/repo-b', source: 'registered' },
        ];
      case 'list_workspaces':
        return [
          { id: 'ws-1', path: '/ws-1', taskTitle: 'Task', repos: ['repo-a'], active: true },
          { id: 'ws-2', path: '/ws-2', taskTitle: '', repos: ['repo-a'], active: false },
        ];
      case 'unregister_profile': {
        const path = (args as { path: string }).path;
        overrides?.onUnregister?.(path);
        profiles = profiles.filter((p) => p.path !== path);
        return null;
      }
      default:
        throw new Error(`unexpected command ${cmd}`);
    }
  });
}

interface Setup {
  manager: ProfileManager;
  callbacks: {
    getActiveProfilePath: ReturnType<typeof vi.fn>;
    setActiveProfilePath: ReturnType<typeof vi.fn>;
    getOpenSessionsForProfile: ReturnType<typeof vi.fn>;
    onClose: ReturnType<typeof vi.fn>;
  };
}

async function setup(view: 'list' | 'new' = 'list', activePath = '/profiles/work'): Promise<Setup> {
  const callbacks = {
    getActiveProfilePath: vi.fn(() => activePath as string | undefined),
    setActiveProfilePath: vi.fn(),
    getOpenSessionsForProfile: vi.fn(() => [] as { title: string }[]),
    onClose: vi.fn(),
  };
  const manager = new ProfileManager(callbacks as unknown as ProfileManagerCallbacks);
  document.body.appendChild(manager.getElement());
  await manager.load(view);
  // Let the focused profile's detail (repos + workspaces) settle.
  await Promise.resolve();
  await Promise.resolve();
  return { manager, callbacks };
}

function windowKey(key: string): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
}

function masterItems(): HTMLElement[] {
  return Array.from(document.querySelectorAll('.pm-master-item'));
}

function focusedName(): string | null {
  return document.querySelector('.pm-master-focused .pm-name')?.textContent ?? null;
}

async function flushDialog(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe('ProfileManager', () => {
  afterEach(() => {
    document.body.innerHTML = '';
    vi.clearAllMocks();
  });

  it('lists all profiles plus the pinned New Profile row', async () => {
    mockBackend();
    const { manager } = await setup();

    const rows = masterItems();
    expect(rows).toHaveLength(3);
    expect(rows[0].textContent).toContain('Work');
    expect(rows[0].textContent).toContain('2 repos');
    expect(rows[1].textContent).toContain('Personal');
    expect(rows[2].classList.contains('pm-new-row')).toBe(true);
    expect(rows[2].textContent).toContain('New Profile…');
    manager.unload();
  });

  it('focuses the active profile on load and marks it with the green dot', async () => {
    mockBackend();
    const { manager } = await setup('list', '/profiles/work');

    expect(focusedName()).toBe('Work');
    const dots = document.querySelectorAll('.pm-master-item .status-dot');
    expect(dots[0].classList.contains('clean')).toBe(true);
    expect(dots[1].classList.contains('pending')).toBe(true);
    manager.unload();
  });

  it('shows the focused profile detail with badges and the repo source table', async () => {
    mockBackend();
    const { manager } = await setup();
    // Detail data resolves asynchronously and patches the panel.
    await flushDialog();

    const detail = document.querySelector('.pm-detail')!;
    expect(detail.querySelector('.pm-detail-title')?.textContent).toBe('Work');
    expect(detail.querySelector('.pm-detail-path')?.textContent).toBe('/profiles/work');
    expect(detail.textContent).toContain('2 workspaces');
    expect(detail.textContent).toContain('1 active task');

    const rows = Array.from(document.querySelectorAll('.pm-repo-table tbody tr'));
    expect(rows).toHaveLength(2);
    expect(rows[0].textContent).toContain('repo-a');
    expect(rows[0].textContent).toContain('auto-discovered');
    expect(rows[1].textContent).toContain('registered');
    manager.unload();
  });

  it('navigates with j/k across profiles and the pinned row', async () => {
    mockBackend();
    const { manager } = await setup();

    windowKey('j');
    expect(focusedName()).toBe('Personal');
    windowKey('j');
    expect(focusedName()).toBe('New Profile…');
    windowKey('j'); // clamped at the pinned row
    expect(focusedName()).toBe('New Profile…');
    windowKey('k');
    windowKey('k');
    expect(focusedName()).toBe('Work');
    manager.unload();
  });

  it('Enter sets the focused profile active and closes', async () => {
    mockBackend();
    const { manager, callbacks } = await setup();

    windowKey('j'); // Personal
    windowKey('Enter');

    expect(callbacks.setActiveProfilePath).toHaveBeenCalledWith('/profiles/personal');
    expect(callbacks.onClose).toHaveBeenCalledOnce();
    manager.unload();
  });

  it('Set Active is disabled for the already-active profile', async () => {
    mockBackend();
    const { manager } = await setup('list', '/profiles/work');
    await flushDialog();

    expect((document.querySelector('.pm-set-active-btn') as HTMLButtonElement).disabled).toBe(true);
    manager.unload();
  });

  it("load('new') focuses the pinned row and shows the inline form", async () => {
    mockBackend();
    const { manager } = await setup('new');

    expect(focusedName()).toBe('New Profile…');
    expect(document.querySelector('.pm-form')).not.toBeNull();
    expect(document.querySelector('.pm-detail-title')?.textContent).toBe('New Profile');
    manager.unload();
  });

  it('d shows a blocking notice (no cancel) when the profile has open sessions', async () => {
    mockBackend();
    const { manager, callbacks } = await setup();
    callbacks.getOpenSessionsForProfile.mockReturnValue([{ title: 'Fix login bug' }]);

    windowKey('d');
    await flushDialog();

    const dialog = document.querySelector('.confirmation-dialog')!;
    expect(dialog.textContent).toContain('Cannot delete profile "Work"');
    expect(dialog.textContent).toContain('Fix login bug');
    expect(dialog.querySelector('.confirmation-dialog-cancel')).toBeNull();

    (dialog.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement).click();
    await flushDialog();

    expect(invokeMock).not.toHaveBeenCalledWith('unregister_profile', expect.anything());
    expect(masterItems()).toHaveLength(3);
    manager.unload();
  });

  it('d confirms, unregisters, and reloads the list', async () => {
    const unregistered: string[] = [];
    mockBackend({ onUnregister: (path) => unregistered.push(path) });
    const { manager, callbacks } = await setup('list', '/profiles/work');

    windowKey('j'); // Personal (not active)
    windowKey('d');
    await flushDialog();

    const dialog = document.querySelector('.confirmation-dialog')!;
    expect(dialog.textContent).toContain('Delete profile "Personal"?');
    expect(dialog.textContent).toContain('remains on disk');

    (dialog.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement).click();
    await flushDialog();
    await flushDialog();

    expect(unregistered).toEqual(['/profiles/personal']);
    expect(masterItems()).toHaveLength(2); // Work + pinned row
    expect(callbacks.setActiveProfilePath).not.toHaveBeenCalled();
    expect(callbacks.onClose).not.toHaveBeenCalled();
    manager.unload();
  });

  it('promotes the first remaining profile when the active one is deleted', async () => {
    mockBackend();
    const { manager, callbacks } = await setup('list', '/profiles/work');

    windowKey('d'); // delete Work (active)
    await flushDialog();
    (document.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement).click();
    await flushDialog();
    await flushDialog();

    expect(callbacks.setActiveProfilePath).toHaveBeenCalledWith('/profiles/personal');
    expect(callbacks.onClose).not.toHaveBeenCalled();
    manager.unload();
  });

  it('closes back to the hub after the last profile is deleted', async () => {
    mockBackend({ profiles: [PROFILES[0]] });
    const { manager, callbacks } = await setup('list', '/profiles/work');

    windowKey('d');
    await flushDialog();
    (document.querySelector('.confirmation-dialog-confirm') as HTMLButtonElement).click();
    await flushDialog();
    await flushDialog();

    expect(callbacks.setActiveProfilePath).toHaveBeenCalledWith(null);
    expect(callbacks.onClose).toHaveBeenCalledOnce();
    manager.unload();
  });

  it('unload removes the window key handler', async () => {
    mockBackend();
    const { manager } = await setup();
    manager.unload();

    windowKey('j');
    expect(document.querySelector('.pm-master-focused')).toBeNull();
  });
});
