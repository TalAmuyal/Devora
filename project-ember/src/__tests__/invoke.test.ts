import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { invoke, invokeLogOnly } from '../invoke';
import { clearErrorBanners, scrapeErrors } from '../errors';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
  Channel: class {},
}));

const mockedInvoke = vi.mocked(tauriInvoke);

function getBanners(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('.error-banner-stack .ws-error-notification')];
}

beforeEach(() => {
  clearErrorBanners();
  scrapeErrors();
  mockedInvoke.mockClear();
  mockedInvoke.mockImplementation(() => Promise.resolve(undefined));
});

describe('invoke (default wrapper)', () => {
  it('passes command and args through and resolves with the backend value', async () => {
    mockedInvoke.mockImplementation(() => Promise.resolve(['a', 'b']));
    const result = await invoke<string[]>('list_profiles', { profilePath: '/p' });
    expect(result).toEqual(['a', 'b']);
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith('list_profiles', { profilePath: '/p' });
  });

  it('surfaces a rejection as a banner, records it, logs it, and rethrows', async () => {
    mockedInvoke.mockImplementation((cmd) =>
      cmd === 'create_workspace' ? Promise.reject('disk full') : Promise.resolve(undefined),
    );

    await expect(invoke('create_workspace', { taskName: 't' })).rejects.toBe('disk full');

    const banners = getBanners();
    expect(banners).toHaveLength(1);
    expect(banners[0].textContent).toContain('create_workspace failed: disk full');
    expect(scrapeErrors()).toEqual(['create_workspace failed: disk full']);
    expect(mockedInvoke).toHaveBeenCalledWith('log_error', {
      level: 'ERROR',
      message: 'create_workspace failed: disk full',
    });
  });

  it('stringifies Error rejections sanely', async () => {
    mockedInvoke.mockImplementation((cmd) =>
      cmd === 'bad_cmd' ? Promise.reject(new Error('exploded')) : Promise.resolve(undefined),
    );

    await expect(invoke('bad_cmd')).rejects.toThrow('exploded');
    expect(getBanners()[0].textContent).toContain('bad_cmd failed: Error: exploded');
  });
});

describe('invokeLogOnly (explicit opt-out)', () => {
  it('logs a rejection as WARN without a banner or recorded error, and rethrows', async () => {
    mockedInvoke.mockImplementation((cmd) =>
      cmd === 'write_pty' ? Promise.reject('pty gone') : Promise.resolve(undefined),
    );

    await expect(invokeLogOnly('write_pty', { id: 1 })).rejects.toBe('pty gone');

    expect(getBanners()).toHaveLength(0);
    expect(scrapeErrors()).toEqual([]);
    const logCalls = mockedInvoke.mock.calls.filter(([cmd]) => cmd === 'log_error');
    expect(logCalls).toEqual([
      ['log_error', { level: 'WARN', message: 'write_pty failed: pty gone' }],
    ]);
  });

  it('resolves transparently on success', async () => {
    mockedInvoke.mockImplementation(() => Promise.resolve(42));
    await expect(invokeLogOnly<number>('resize_pty', { cols: 80 })).resolves.toBe(42);
    expect(mockedInvoke).toHaveBeenCalledWith('resize_pty', { cols: 80 });
  });
});
