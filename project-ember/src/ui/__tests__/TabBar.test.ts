import { describe, it, expect, vi } from 'vitest';
import { TabBar } from '../TabBar';
import type { SessionManager } from '../../session/SessionManager';

describe('TabBar global hold', () => {
  it('keeps the Paused hint across session rerenders and clears it on resume', () => {
    const sessions = {
      onChange: vi.fn(), getSessions: () => [], getActiveSessionId: () => null,
    } as unknown as SessionManager;
    const host = document.createElement('div');
    const bar = new TabBar(host, sessions);
    bar.render();
    expect(host.querySelector('.global-pause-hint')).toBeNull();
    bar.setPaused(true);
    expect(host.querySelector('.global-pause-hint')?.textContent).toBe('Paused');
    bar.render();
    expect(host.querySelector('.global-pause-hint')?.textContent).toBe('Paused');
    bar.setPaused(false);
    expect(host.querySelector('.global-pause-hint')).toBeNull();
  });
});
