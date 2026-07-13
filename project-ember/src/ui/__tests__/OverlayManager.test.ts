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

  it('applies the optional overlayClass to the wrapper element', () => {
    const { manager, appEl } = makeManager();

    manager.showTabCoveringOverlay(
      document.createElement('div'),
      undefined,
      undefined,
      'overlay-passthrough',
    );

    const wrapper = appEl.querySelector('.overlay-tab-covering');
    expect(wrapper).not.toBeNull();
    expect(wrapper?.classList.contains('overlay-passthrough')).toBe(true);
  });

  it('omits any extra class when overlayClass is not provided', () => {
    const { manager, appEl } = makeManager();

    manager.showTabCoveringOverlay(document.createElement('div'));

    const wrapper = appEl.querySelector('.overlay-tab-covering') as HTMLElement;
    expect(wrapper.className).toBe('overlay-tab-covering');
  });

  it('removes the overlay element from appEl on dismissal', () => {
    const { manager, appEl } = makeManager();

    manager.showTabCoveringOverlay(document.createElement('div'), vi.fn());
    expect(appEl.querySelector('.overlay-tab-covering')).not.toBeNull();

    manager.dismissTabCoveringOverlay();
    expect(appEl.querySelector('.overlay-tab-covering')).toBeNull();
  });
});

describe('OverlayManager tab-covering user-dismiss override', () => {
  it('invokes the override instead of dismissing on dismissActiveOverlay', () => {
    const { manager } = makeManager();
    const cleanup = vi.fn();
    const onUserDismiss = vi.fn();

    manager.showTabCoveringOverlay(
      document.createElement('div'),
      cleanup,
      undefined,
      undefined,
      onUserDismiss,
    );
    const handled = manager.dismissActiveOverlay();

    expect(handled).toBe(true);
    expect(onUserDismiss).toHaveBeenCalledOnce();
    expect(cleanup).not.toHaveBeenCalled();
    expect(manager.isTabCoveringOverlayActive()).toBe(true);
  });

  it('dismissTabCoveringOverlay force-dismisses despite an override', () => {
    const { manager } = makeManager();
    const cleanup = vi.fn();
    const onUserDismiss = vi.fn();

    manager.showTabCoveringOverlay(
      document.createElement('div'),
      cleanup,
      undefined,
      undefined,
      onUserDismiss,
    );
    manager.dismissTabCoveringOverlay();

    expect(onUserDismiss).not.toHaveBeenCalled();
    expect(cleanup).toHaveBeenCalledOnce();
    expect(manager.isTabCoveringOverlayActive()).toBe(false);
  });

  it('lets the override replace the overlay with another one', () => {
    const { manager } = makeManager();
    const firstCleanup = vi.fn();
    const secondCleanup = vi.fn();
    const onUserDismiss = vi.fn(() => {
      manager.showTabCoveringOverlay(document.createElement('div'), secondCleanup);
    });

    manager.showTabCoveringOverlay(
      document.createElement('div'),
      firstCleanup,
      undefined,
      undefined,
      onUserDismiss,
    );
    manager.dismissActiveOverlay();

    expect(firstCleanup).toHaveBeenCalledOnce();
    expect(secondCleanup).not.toHaveBeenCalled();
    expect(manager.isTabCoveringOverlayActive()).toBe(true);

    // The replacement overlay has no override, so the next user dismissal closes it.
    manager.dismissActiveOverlay();
    expect(secondCleanup).toHaveBeenCalledOnce();
    expect(manager.isTabCoveringOverlayActive()).toBe(false);
  });

  it('clears the override when the overlay is replaced', () => {
    const { manager } = makeManager();
    const onUserDismiss = vi.fn();

    manager.showTabCoveringOverlay(
      document.createElement('div'),
      undefined,
      undefined,
      undefined,
      onUserDismiss,
    );
    manager.showTabCoveringOverlay(document.createElement('div'));
    manager.dismissActiveOverlay();

    expect(onUserDismiss).not.toHaveBeenCalled();
    expect(manager.isTabCoveringOverlayActive()).toBe(false);
  });
});

describe('OverlayManager tab-covering focus acquisition', () => {
  it('moves keyboard focus onto the overlay wrapper when shown', () => {
    const appEl = document.createElement('div');
    document.body.appendChild(appEl);
    const manager = new OverlayManager(appEl);

    // Simulate the terminal textarea holding focus before the overlay opens.
    const textarea = document.createElement('textarea');
    document.body.appendChild(textarea);
    textarea.focus();
    expect(document.activeElement).toBe(textarea);

    manager.showTabCoveringOverlay(document.createElement('div'));

    const wrapper = appEl.querySelector('.overlay-tab-covering');
    expect(document.activeElement).toBe(wrapper);

    manager.dismissTabCoveringOverlay();
    appEl.remove();
    textarea.remove();
  });
});

describe('OverlayManager tab-covering focus restoration', () => {
  it('restores focus to the provided Focusable on dismissTabCoveringOverlay', () => {
    const { manager } = makeManager();
    const focusable = { focus: vi.fn() };

    manager.showTabCoveringOverlay(document.createElement('div'), undefined, focusable);
    manager.dismissTabCoveringOverlay();

    expect(focusable.focus).toHaveBeenCalledOnce();
  });

  it('restores focus via dismissActiveOverlay', () => {
    const { manager } = makeManager();
    const focusable = { focus: vi.fn() };

    manager.showTabCoveringOverlay(document.createElement('div'), undefined, focusable);
    manager.dismissActiveOverlay();

    expect(focusable.focus).toHaveBeenCalledOnce();
  });

  it('restores focus only after the overlay element has been removed', () => {
    const { manager, appEl } = makeManager();
    const focusable = {
      focus: vi.fn(() => {
        expect(appEl.querySelector('.overlay-tab-covering')).toBeNull();
      }),
    };

    manager.showTabCoveringOverlay(document.createElement('div'), undefined, focusable);
    manager.dismissTabCoveringOverlay();

    expect(focusable.focus).toHaveBeenCalledOnce();
  });

  it('runs the cleanup hook before restoring focus', () => {
    const { manager } = makeManager();
    const calls: string[] = [];
    const cleanup = vi.fn(() => calls.push('cleanup'));
    const focusable = { focus: vi.fn(() => calls.push('focus')) };

    manager.showTabCoveringOverlay(document.createElement('div'), cleanup, focusable);
    manager.dismissTabCoveringOverlay();

    expect(calls).toEqual(['cleanup', 'focus']);
  });

  it('dismisses cleanly when no Focusable is provided', () => {
    const { manager } = makeManager();

    manager.showTabCoveringOverlay(document.createElement('div'));

    expect(() => manager.dismissTabCoveringOverlay()).not.toThrow();
  });
});
