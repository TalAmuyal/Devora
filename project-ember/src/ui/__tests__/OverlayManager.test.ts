import { describe, it, expect, vi } from 'vitest';
import { OverlayManager } from '../OverlayManager';

function makeManager(): { manager: OverlayManager; appEl: HTMLElement } {
  const appEl = document.createElement('div');
  return { manager: new OverlayManager(appEl), appEl };
}

describe('OverlayManager tab-covering cleanup hook', () => {
  it('invokes the cleanup hook exactly once on dismissTabCoveringOverlay', () => {
    const { manager } = makeManager();
    const cleanup = vi.fn();

    manager.showTabCoveringOverlay(document.createElement('div'), cleanup);
    manager.dismissTabCoveringOverlay();

    expect(cleanup).toHaveBeenCalledOnce();
  });

  it('invokes the tab-covering cleanup hook via dismissActiveOverlay', () => {
    const { manager } = makeManager();
    const cleanup = vi.fn();

    manager.showTabCoveringOverlay(document.createElement('div'), cleanup);
    const dismissed = manager.dismissActiveOverlay();

    expect(dismissed).toBe(true);
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it('does not recurse or double-invoke when a hook re-enters dismissTabCoveringOverlay', () => {
    const { manager } = makeManager();
    const cleanup = vi.fn(() => {
      manager.dismissTabCoveringOverlay();
    });

    manager.showTabCoveringOverlay(document.createElement('div'), cleanup);
    manager.dismissTabCoveringOverlay();

    expect(cleanup).toHaveBeenCalledOnce();
  });

  it('runs the previous hook when a second tab-covering overlay is shown', () => {
    const { manager } = makeManager();
    const firstCleanup = vi.fn();
    const secondCleanup = vi.fn();

    manager.showTabCoveringOverlay(document.createElement('div'), firstCleanup);
    manager.showTabCoveringOverlay(document.createElement('div'), secondCleanup);

    expect(firstCleanup).toHaveBeenCalledOnce();
    expect(secondCleanup).not.toHaveBeenCalled();

    manager.dismissTabCoveringOverlay();

    expect(secondCleanup).toHaveBeenCalledOnce();
    expect(firstCleanup).toHaveBeenCalledOnce();
  });

  it('dismisses cleanly when shown with no cleanup hook', () => {
    const { manager } = makeManager();

    manager.showTabCoveringOverlay(document.createElement('div'));

    expect(() => manager.dismissTabCoveringOverlay()).not.toThrow();
    expect(manager.isTabCoveringOverlayActive()).toBe(false);
  });

  it('removes the overlay element from appEl on dismissal', () => {
    const { manager, appEl } = makeManager();

    manager.showTabCoveringOverlay(document.createElement('div'), vi.fn());
    expect(appEl.querySelector('.overlay-tab-covering')).not.toBeNull();

    manager.dismissTabCoveringOverlay();
    expect(appEl.querySelector('.overlay-tab-covering')).toBeNull();
  });
});
