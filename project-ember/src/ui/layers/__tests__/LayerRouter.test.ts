import { describe, it, expect, vi, afterEach } from 'vitest';
import { LayerRouter } from '../LayerRouter';
import { LayerStack } from '../LayerStack';
import { pageLayer, popupLayer, terminalLayer } from '../presets';

/**
 * The three-stage routing order (ADR-003): app-wide shortcuts → window stack → active tab stack.
 * This is where the ordering itself is pinned; `LayerStack.test.ts` covers what each stack does with a key.
 */

const routers: LayerRouter[] = [];
const spies: Array<(e: KeyboardEvent) => void> = [];

interface Built {
  router: LayerRouter;
  windowStack: LayerStack;
  tabStack: LayerStack;
}

function build(opts: { withTab?: boolean } = {}): Built {
  const windowHost = document.createElement('div');
  const tabHost = document.createElement('div');
  document.body.append(windowHost, tabHost);
  const windowStack = new LayerStack({ host: windowHost });
  const tabStack = new LayerStack({ host: tabHost });
  const router = new LayerRouter({
    windowStack,
    resolveTabStack: () => (opts.withTab === false ? null : tabStack),
  });
  router.install();
  routers.push(router);
  return { router, windowStack, tabStack };
}

function content(name: string): HTMLElement {
  const el = document.createElement('div');
  el.className = `${name}-content`;
  return el;
}

function dispatch(init: KeyboardEventInit): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(e);
  return e;
}

/** A window-capture listener registered *after* the router, so it only runs on keys the router did not consume. */
function listenerAfterRouter(): ReturnType<typeof vi.fn> {
  const spy = vi.fn();
  window.addEventListener('keydown', spy, true);
  spies.push(spy);
  return spy;
}

afterEach(() => {
  for (const r of routers) r.uninstall();
  routers.length = 0;
  for (const spy of spies) window.removeEventListener('keydown', spy, true);
  spies.length = 0;
  document.body.innerHTML = '';
});

describe('LayerRouter — ordering', () => {
  it('runs the shortcut handler before any layer sees the key', () => {
    const { router, windowStack } = build();
    const order: string[] = [];
    router.setShortcutHandler(() => {
      order.push('shortcut');
      return true;
    });
    windowStack.push(
      pageLayer({
        name: 'ws-hub',
        element: content('ws-hub'),
        onKey: () => {
          order.push('hub');
          return true;
        },
      }),
    );

    const e = dispatch({ key: '1', code: 'Digit1', ctrlKey: true });

    // The hub matches bare '1' as a category filter; running shortcuts first is what stops it shadowing Ctrl+1.
    expect(order).toEqual(['shortcut']);
    expect(e.defaultPrevented).toBe(true);
  });

  it('falls through to the window stack when no shortcut claims the key', () => {
    const { router, windowStack } = build();
    router.setShortcutHandler(() => false);
    const onKey = vi.fn(() => true);
    windowStack.push(pageLayer({ name: 'ws-hub', element: content('ws-hub'), onKey }));

    dispatch({ key: 'j', code: 'KeyJ' });
    expect(onKey).toHaveBeenCalledOnce();
  });

  it('reaches the tab stack when the window stack is empty', () => {
    const { tabStack } = build();
    const onKey = vi.fn(() => true);
    tabStack.push(pageLayer({ name: 'crit-review', element: content('crit'), onKey }));

    const e = dispatch({ key: 'j', code: 'KeyJ' });
    expect(onKey).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('reaches the tab stack through a transparent window layer', () => {
    const { windowStack, tabStack } = build();
    const anchor = document.createElement('div');
    document.body.appendChild(anchor);
    const popup = content('menu');
    anchor.appendChild(popup);
    windowStack.push(popupLayer({ name: 'dropdown-menu', element: popup }));
    const onKey = vi.fn(() => true);
    tabStack.push(pageLayer({ name: 'crit-review', element: content('crit'), onKey }));

    dispatch({ key: 'j', code: 'KeyJ' });
    expect(onKey).toHaveBeenCalledOnce();
  });

  it('an opaque window layer stops routing before the tab stack', () => {
    const { windowStack, tabStack } = build();
    windowStack.push(pageLayer({ name: 'ws-hub', element: content('ws-hub') }));
    const onKey = vi.fn(() => true);
    tabStack.push(pageLayer({ name: 'crit-review', element: content('crit'), onKey }));

    dispatch({ key: 'j', code: 'KeyJ' });
    expect(onKey).not.toHaveBeenCalled();
  });

  it('does nothing when there is no tab stack', () => {
    build({ withTab: false });
    const after = listenerAfterRouter();

    dispatch({ key: 'j', code: 'KeyJ' });
    expect(after).toHaveBeenCalledOnce();
  });
});

describe('LayerRouter — consumption', () => {
  it('stops immediate propagation on a consumed key', () => {
    const { windowStack } = build();
    windowStack.push(pageLayer({ name: 'a', element: content('a') }));
    const after = listenerAfterRouter();

    dispatch({ key: 'Escape', code: 'Escape' });
    expect(after).not.toHaveBeenCalled();
  });

  it('leaves a blocked key alone so it can reach the focused element', () => {
    const { tabStack } = build();
    tabStack.push(terminalLayer({ name: 'terminal', element: content('terminal') }));
    const after = listenerAfterRouter();

    // The terminal layer is opaque but consumes nothing: this is how typing reaches xterm's textarea.
    const e = dispatch({ key: 'q', code: 'KeyQ' });
    expect(e.defaultPrevented).toBe(false);
    expect(after).toHaveBeenCalledOnce();
  });

  it('always runs the key observer, even on consumed keys', () => {
    const { router, windowStack } = build();
    const observer = vi.fn();
    router.setKeyObserver(observer);
    windowStack.push(pageLayer({ name: 'a', element: content('a') }));

    dispatch({ key: 'Escape', code: 'Escape' });
    expect(observer).toHaveBeenCalledOnce();
  });

  it('uninstall stops routing entirely', () => {
    const { router, windowStack } = build();
    const onKey = vi.fn(() => true);
    windowStack.push(pageLayer({ name: 'a', element: content('a'), onKey }));

    router.uninstall();
    dispatch({ key: 'j', code: 'KeyJ' });
    expect(onKey).not.toHaveBeenCalled();
  });
});
