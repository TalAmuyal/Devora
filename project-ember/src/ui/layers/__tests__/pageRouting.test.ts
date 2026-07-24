import { describe, it, expect, vi, afterEach } from 'vitest';
import { LayerStack, LayerStackDeps } from '../LayerStack';
import type { DismissDecision, LayerHandle, LayerSpec } from '../types';
import { KeyboardShortcuts } from '../../KeyboardShortcuts';
import type { SessionManager } from '../../../session/SessionManager';

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
 * Push the Settings Hub over whatever is open, mirroring `main.ts`: dismissal is the default 'close', so it pops the page — revealing the live hub beneath when one exists, or the base (the session) when opened from the palette.
 */
function openSettings(stack: LayerStack): LayerHandle {
  return stack.push({
    name: 'settings-hub',
    kind: 'page',
    element: content('settings-hub'),
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

  it('dismissing Settings opened with no hub beneath closes it, revealing the base (the session)', () => {
    const baseFocus = { focus: vi.fn() };
    const stack = makeStack({ resolveBaseFocus: () => baseFocus });
    openSettings(stack); // palette path: nothing beneath

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(stack.find('settings-hub')).toBeNull();
    expect(stack.isEmpty()).toBe(true);
    expect(baseFocus.focus).toHaveBeenCalledOnce();
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

  it('wired to KeyboardShortcuts like main.ts, a page admits F1 + font but blocks new session and tab nav', () => {
    const createSession = vi.fn();
    const activateNext = vi.fn();
    const onOpenUserGuide = vi.fn();
    const sessionManager = {
      createSession,
      activateNext,
      activatePrevious: vi.fn(),
      moveTabForward: vi.fn(),
      moveTabBackward: vi.fn(),
      getActiveSession: () => null,
      getSessions: () => [],
    } as unknown as SessionManager;
    const shortcuts = new KeyboardShortcuts(sessionManager, vi.fn(), vi.fn(), onOpenUserGuide);

    const stack = makeStack();
    stack.setGlobalHandler((e) => shortcuts.handleGlobal(e));
    stack.setPageBarrierAdmits((e) => shortcuts.allowedThroughPageBarrier(e));
    stack.push(hubSpec());

    document.documentElement.style.fontSize = '';
    dispatch({ key: 'F1', code: 'F1' });
    expect(onOpenUserGuide).toHaveBeenCalledOnce();
    dispatch({ key: '1', code: 'Digit1', ctrlKey: true });
    expect(document.documentElement.style.fontSize).toBe('12px');

    dispatch({ key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true });
    dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });
    expect(createSession).not.toHaveBeenCalled();
    expect(activateNext).not.toHaveBeenCalled();
  });
});
