import { describe, it, expect, vi, afterEach } from 'vitest';
import { LayerStack, LayerStackDeps } from '../LayerStack';
import type { DismissDecision, LayerHandle, LayerSpec } from '../types';

/**
 * Integration tests for the page layer contracts that `main.ts` wires (ADR-003): the hub/Settings/palette dismissal semantics, cheatsheet single-fire, the zero-profile veto, and the modality barrier over base shortcuts.
 * These pin the wiring decisions that `main.ts` itself is not unit-testable for.
 */

const liveStacks: LayerStack[] = [];
const windowSpies: Array<(e: KeyboardEvent) => void> = [];

function makeStack(deps: Partial<LayerStackDeps> = {}): LayerStack {
  const pageHost = document.createElement('div');
  document.body.appendChild(pageHost);
  const stack = new LayerStack({
    pageHost,
    modalHost: document.body,
    resolveBaseFocus: () => null,
    ...deps,
  });
  stack.install();
  liveStacks.push(stack);
  return stack;
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

/** A window-capture listener registered after the stack, so it only fires on keys the stack did not consume. */
function listenerAfterStack(): ReturnType<typeof vi.fn> {
  const spy = vi.fn();
  window.addEventListener('keydown', spy, true);
  windowSpies.push(spy);
  return spy;
}

/** The Workspace Hub page spec, mirroring `main.ts` (default dismissal closes; a caller can inject the cheatsheet/zero-profile decision). */
function hubSpec(opts: { onReveal?: () => void; dismiss?: () => DismissDecision } = {}): LayerSpec {
  return {
    name: 'ws-hub',
    kind: 'page',
    element: content('ws-hub'),
    onReveal: opts.onReveal,
    onUserDismissRequest: opts.dismiss ?? (() => 'close'),
  };
}

/**
 * Push the Settings Hub over whatever is open, mirroring `main.ts`: dismissal pops back to a live hub when one is beneath, otherwise atomically replaces itself with a fresh hub.
 */
function openSettings(stack: LayerStack): LayerHandle {
  return stack.push({
    name: 'settings-hub',
    kind: 'page',
    element: content('settings-hub'),
    onUserDismissRequest: () => {
      if (stack.find('ws-hub')) return 'close';
      stack.replaceTop(hubSpec());
      return 'handled';
    },
  });
}

afterEach(() => {
  for (const s of liveStacks) {
    s.clear();
    s.uninstall();
  }
  liveStacks.length = 0;
  for (const spy of windowSpies) {
    window.removeEventListener('keydown', spy, true);
  }
  windowSpies.length = 0;
  document.body.innerHTML = '';
});

describe('page routing — Settings over the hub', () => {
  it('P opens Settings over the hub; q pops back and refreshes the revealed hub', () => {
    const stack = makeStack();
    const onReveal = vi.fn();
    const hub = stack.push(hubSpec({ onReveal }));

    const settings = openSettings(stack);
    expect(stack.top()).toBe(settings);
    expect(stack.depth()).toBe(2);
    expect(stack.find('ws-hub')).toBe(hub);

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(stack.find('settings-hub')).toBeNull();
    expect(stack.top()).toBe(hub);
    expect(onReveal).toHaveBeenCalledOnce();
  });

  it('dismissing Settings opened with no hub beneath replaces it with a fresh hub', () => {
    const stack = makeStack();
    openSettings(stack); // palette path: nothing beneath

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(stack.find('settings-hub')).toBeNull();
    expect(stack.find('ws-hub')).not.toBeNull();
    expect(stack.depth()).toBe(1);
  });
});

describe('page routing — hub dismissal decisions', () => {
  it('the zero-profile hub vetoes q and stays open', () => {
    const stack = makeStack();
    stack.push(hubSpec({ dismiss: () => 'veto' }));

    const e = dispatch({ key: 'q', code: 'KeyQ' });

    expect(stack.find('ws-hub')).not.toBeNull();
    expect(stack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
  });

  it('closing the hub cheatsheet with q fires once, consumes the key, and keeps the hub', () => {
    const stack = makeStack();
    let cheatsheetOpen = true;
    const dismiss = vi.fn((): DismissDecision => {
      if (cheatsheetOpen) {
        cheatsheetOpen = false;
        return 'handled';
      }
      return 'close';
    });
    stack.push(hubSpec({ dismiss }));
    const after = listenerAfterStack();

    const e = dispatch({ key: 'q', code: 'KeyQ' });

    expect(dismiss).toHaveBeenCalledOnce();
    expect(cheatsheetOpen).toBe(false);
    expect(stack.depth()).toBe(1); // the hub is not closed by the same press (fixes the double-fire)
    expect(e.defaultPrevented).toBe(true);
    expect(after).not.toHaveBeenCalled();
  });
});

describe('page routing — palette Ctrl+S no-op', () => {
  it('the palette consumes Ctrl+S without closing (mutual exclusion with the hub)', () => {
    const stack = makeStack();
    stack.push({
      name: 'command-palette',
      kind: 'page',
      element: content('command-palette'),
      onKey: (ev) => ev.ctrlKey && !ev.shiftKey && ev.code === 'KeyS',
    });

    const e = dispatch({ key: 's', code: 'KeyS', ctrlKey: true });

    expect(stack.find('command-palette')).not.toBeNull();
    expect(stack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
  });
});

describe('page routing — modality barrier over base shortcuts', () => {
  it('Ctrl+ArrowRight is blocked under a page but reaches the base shortcuts on an empty stack', () => {
    const globalHandler = vi.fn(() => true);
    const stack = makeStack();
    stack.setGlobalHandler(globalHandler);
    const hub = stack.push(hubSpec());

    dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });
    expect(globalHandler).not.toHaveBeenCalled();

    stack.remove(hub);
    const e = dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });
    expect(globalHandler).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });
});
