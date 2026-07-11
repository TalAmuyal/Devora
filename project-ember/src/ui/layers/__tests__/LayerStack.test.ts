import { describe, it, expect, vi, afterEach } from 'vitest';
import { LayerStack, LayerStackDeps } from '../LayerStack';
import type { Focusable, LayerKind, LayerSpec } from '../types';

const liveStacks: LayerStack[] = [];
const windowSpies: Array<(e: KeyboardEvent) => void> = [];

interface Built {
  stack: LayerStack;
  pageHost: HTMLElement;
}

function makeStack(deps: Partial<LayerStackDeps> = {}): Built {
  const pageHost = document.createElement('div');
  pageHost.className = 'page-host';
  document.body.appendChild(pageHost);
  const stack = new LayerStack({
    pageHost,
    modalHost: document.body,
    resolveBaseFocus: () => null,
    ...deps,
  });
  stack.install();
  liveStacks.push(stack);
  return { stack, pageHost };
}

/** A layer spec with a fresh content element. `place` mounts the element for popup/panel kinds, which the caller owns. */
function spec(name: string, kind: LayerKind, over: Partial<LayerSpec> = {}, place?: HTMLElement): LayerSpec {
  const element = document.createElement('div');
  element.className = `${name}-content`;
  if (place) place.appendChild(element);
  return { name, kind, element, ...over };
}

function dispatch(init: KeyboardEventInit): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(e);
  return e;
}

/** A window-capture listener registered *after* the stack, so it only runs on keys the stack did not consume. */
function listenerAfterStack(): ReturnType<typeof vi.fn> {
  const spy = vi.fn();
  window.addEventListener('keydown', spy, true);
  windowSpies.push(spy);
  return spy;
}

function focusableSpy(): Focusable & { focus: ReturnType<typeof vi.fn> } {
  return { focus: vi.fn() };
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

describe('LayerStack — stack operations', () => {
  it('pushes onto the top and reports depth/top/isEmpty', () => {
    const { stack } = makeStack();
    expect(stack.isEmpty()).toBe(true);

    const a = stack.push(spec('a', 'page'));
    const b = stack.push(spec('b', 'page'));

    expect(stack.depth()).toBe(2);
    expect(stack.isEmpty()).toBe(false);
    expect(stack.top()).toBe(b);
    expect(stack.find('a')).toBe(a);
  });

  it('mounts a page wrapper under the page host and a modal wrapper under the modal host', () => {
    const { stack, pageHost } = makeStack();
    const page = stack.push(spec('ws-hub', 'page'));
    const modal = stack.push(spec('confirm', 'modal'));

    expect(page.wrapper.parentElement).toBe(pageHost);
    expect(page.wrapper.classList.contains('overlay-tab-covering')).toBe(true);
    expect(page.wrapper.classList.contains('layer-page')).toBe(true);
    expect(page.wrapper.contains(page.element)).toBe(true);

    expect(modal.wrapper.parentElement).toBe(document.body);
    expect(modal.wrapper.classList.contains('layer-modal')).toBe(true);
    expect(modal.wrapper.classList.contains('confirm-backdrop')).toBe(true);
  });

  it('uses the caller-placed element as its own wrapper for popup and panel kinds', () => {
    const { stack } = makeStack();
    const host = document.createElement('div');
    document.body.appendChild(host);

    const popup = stack.push(spec('menu', 'popup', {}, host));
    expect(popup.wrapper).toBe(popup.element);
    expect(popup.wrapper.parentElement).toBe(host);
  });

  it('applies wrapperClass to the wrapper', () => {
    const { stack } = makeStack();
    const page = stack.push(spec('palette', 'page', { wrapperClass: 'overlay-passthrough' }));
    expect(page.wrapper.classList.contains('overlay-passthrough')).toBe(true);
  });

  it('pops the top and detaches its wrapper', () => {
    const { stack } = makeStack();
    const a = stack.push(spec('a', 'page'));
    stack.push(spec('b', 'page'));

    stack.pop();
    expect(stack.depth()).toBe(1);
    expect(stack.top()).toBe(a);
    expect(stack.find('b')).toBeNull();
  });

  it('inserts a panel at the bottom, never intercepting the page above it', () => {
    const { stack } = makeStack();
    const page = stack.push(spec('ws-hub', 'page'));
    const host = document.createElement('div');
    document.body.appendChild(host);
    const panel = stack.push(spec('crit', 'panel', {}, host));

    expect(stack.depth()).toBe(2);
    expect(stack.top()).toBe(page);
    expect(stack.topOf('panel')).toBe(panel);
  });

  it('removing a non-top layer detaches it without revealing or refocusing', () => {
    const base = focusableSpy();
    const { stack } = makeStack({ resolveBaseFocus: () => base });
    const a = stack.push(spec('a', 'page', { onReveal: vi.fn() }));
    const b = stack.push(spec('b', 'page'));

    stack.remove(a);

    expect(stack.depth()).toBe(1);
    expect(stack.top()).toBe(b);
    expect(a.wrapper.parentElement).toBeNull();
    expect(base.focus).not.toHaveBeenCalled();
  });

  it('removing a page auto-removes a popup anchored inside it and restores base focus', () => {
    const base = focusableSpy();
    const { stack } = makeStack({ resolveBaseFocus: () => base });
    const page = stack.push(spec('ws-hub', 'page'));
    const popupCleanup = vi.fn();
    // The dropdown lives inside the page content, so it is DOM-contained in the page wrapper.
    const popup = stack.push(spec('dropdown', 'popup', { onCleanup: popupCleanup }, page.element));

    stack.remove(page);

    expect(stack.isEmpty()).toBe(true);
    expect(popupCleanup).toHaveBeenCalledOnce();
    expect(popup.wrapper.parentElement).toBeNull();
    expect(base.focus).toHaveBeenCalledOnce();
  });
});

describe('LayerStack — reveal & cleanup', () => {
  it('runs onReveal on the revealed layer when a cover pops', () => {
    const onReveal = vi.fn();
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onReveal }));
    stack.push(spec('b', 'page'));

    stack.pop();
    expect(onReveal).toHaveBeenCalledOnce();
  });

  it('restores focus to the revealed page on a modal pop but does not fire its onReveal', () => {
    const onReveal = vi.fn();
    const focus = focusableSpy();
    const { stack } = makeStack();
    stack.push(spec('page', 'page', { onReveal, resolveFocus: () => focus }));
    stack.push(spec('confirm', 'modal'));

    const before = focus.focus.mock.calls.length;
    stack.pop();

    // A modal is a transient sub-interaction: focus returns to the page, but its navigation refresh does not re-run.
    expect(onReveal).not.toHaveBeenCalled();
    expect(focus.focus.mock.calls.length).toBeGreaterThan(before);
  });

  it('does not reveal the layer below on replaceTop', () => {
    const onRevealA = vi.fn();
    const cleanupB = vi.fn();
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onReveal: onRevealA }));
    stack.push(spec('b', 'page', { onCleanup: cleanupB }));

    const c = stack.replaceTop(spec('c', 'page'));

    expect(cleanupB).toHaveBeenCalledOnce();
    expect(onRevealA).not.toHaveBeenCalled();
    expect(stack.depth()).toBe(2);
    expect(stack.top()).toBe(c);
    expect(stack.find('a')).not.toBeNull();
  });

  it('closing a popup over a page leaves the page undisturbed (no onReveal, no focus move)', () => {
    const onReveal = vi.fn();
    const { stack } = makeStack();
    const page = stack.push(spec('ws-hub', 'page', { onReveal }));
    const focusedInPage = document.createElement('button');
    page.element.appendChild(focusedInPage);
    focusedInPage.focus();
    const popup = stack.push(spec('dropdown', 'popup', {}, page.element));

    stack.remove(popup);

    expect(onReveal).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(focusedInPage);
    expect(stack.top()).toBe(page);
  });

  it('runs cleanup exactly once across pop', () => {
    const onCleanup = vi.fn();
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onCleanup }));

    stack.pop();
    stack.pop(); // no-op: nothing left
    expect(onCleanup).toHaveBeenCalledOnce();
  });

  it('clear() cleans up and detaches every layer once', () => {
    const cleanupA = vi.fn();
    const cleanupB = vi.fn();
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onCleanup: cleanupA }));
    stack.push(spec('b', 'modal', { onCleanup: cleanupB }));

    stack.clear();

    expect(cleanupA).toHaveBeenCalledOnce();
    expect(cleanupB).toHaveBeenCalledOnce();
    expect(stack.isEmpty()).toBe(true);
  });
});

describe('LayerStack — focus', () => {
  it('focuses resolveFocus on push', () => {
    const { stack } = makeStack();
    const target = focusableSpy();
    stack.push(spec('a', 'page', { resolveFocus: () => target }));
    expect(target.focus).toHaveBeenCalledOnce();
  });

  it('focuses the wrapper on push when resolveFocus is absent', () => {
    const { stack } = makeStack();
    const a = stack.push(spec('a', 'page'));
    expect(document.activeElement).toBe(a.wrapper);
  });

  it('does not move focus when a popup is pushed', () => {
    const { stack } = makeStack();
    const sentinel = document.createElement('button');
    document.body.appendChild(sentinel);
    sentinel.focus();

    const host = document.createElement('div');
    document.body.appendChild(host);
    stack.push(spec('menu', 'popup', {}, host));

    expect(document.activeElement).toBe(sentinel);
  });

  it('focuses the revealed layer when the top pops', () => {
    const { stack } = makeStack();
    const a = stack.push(spec('a', 'page'));
    stack.push(spec('b', 'page'));

    stack.pop();
    expect(document.activeElement).toBe(a.wrapper);
  });

  it('focuses a panel pushed onto an empty stack', () => {
    const { stack } = makeStack();
    const host = document.createElement('div');
    document.body.appendChild(host);
    const panel = stack.push(spec('crit', 'panel', {}, host));
    expect(document.activeElement).toBe(panel.wrapper);
  });

  it('does not steal focus when a panel is inserted under an existing page', () => {
    const { stack } = makeStack();
    const page = stack.push(spec('ws-hub', 'page'));
    const host = document.createElement('div');
    document.body.appendChild(host);
    stack.push(spec('crit', 'panel', {}, host));
    expect(document.activeElement).toBe(page.wrapper);
  });

  it('focuses the revealed panel when the page above it pops', () => {
    const { stack } = makeStack();
    const host = document.createElement('div');
    document.body.appendChild(host);
    const panel = stack.push(spec('crit', 'panel', {}, host));
    stack.push(spec('ws-hub', 'page'));

    stack.pop();
    expect(document.activeElement).toBe(panel.wrapper);
  });

  it('focuses resolveBaseFocus resolved at pop time when the stack empties', () => {
    let base: Focusable = focusableSpy();
    const { stack } = makeStack({ resolveBaseFocus: () => base });
    stack.push(spec('a', 'page'));

    // The active session changes while the layer is up; the base is re-resolved at pop time.
    const later = focusableSpy();
    base = later;
    stack.pop();

    expect(later.focus).toHaveBeenCalledOnce();
  });
});

describe('LayerStack — key dispatch', () => {
  it('lets the top layer onKey win and consumes the key', () => {
    const { stack } = makeStack();
    const lowerKey = vi.fn(() => false);
    const topKey = vi.fn(() => true);
    stack.push(spec('a', 'page', { onKey: lowerKey }));
    stack.push(spec('b', 'page', { onKey: topKey }));

    const e = dispatch({ key: 'x', code: 'KeyX' });
    expect(topKey).toHaveBeenCalledOnce();
    expect(lowerKey).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(true);
  });

  it('traps Tab within a modal on top', () => {
    const { stack } = makeStack();
    const modal = spec('confirm', 'modal');
    const first = document.createElement('button');
    const second = document.createElement('button');
    first.id = 'first';
    second.id = 'second';
    modal.element.append(first, second);
    stack.push(modal);
    first.focus();

    const e = dispatch({ key: 'Tab', code: 'Tab' });
    expect(document.activeElement?.id).toBe('second');
    expect(e.defaultPrevented).toBe(true);
  });

  it('does not trap Tab when the top is a page', () => {
    const globalHandler = vi.fn(() => true);
    const { stack } = makeStack();
    stack.setGlobalHandler(globalHandler);
    const page = spec('ws-hub', 'page');
    const button = document.createElement('button');
    page.element.appendChild(button);
    stack.push(page);
    button.focus();

    const e = dispatch({ key: 'Tab', code: 'Tab' });
    // Tab is not a dismiss key, the hub has no onKey, and Tab is not allowed through the page barrier.
    expect(document.activeElement).toBe(button);
    expect(e.defaultPrevented).toBe(false);
    expect(globalHandler).not.toHaveBeenCalled();
  });

  it('Escape pops the top when the dismissal decision is close', () => {
    const { stack } = makeStack();
    stack.push(spec('a', 'page'));
    const b = stack.push(spec('b', 'page', { onUserDismissRequest: () => 'close' }));

    const e = dispatch({ key: 'Escape', code: 'Escape' });
    expect(stack.find('b')).toBeNull();
    expect(stack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
    void b;
  });

  it('q pops the top by default (no onUserDismissRequest)', () => {
    const { stack } = makeStack();
    stack.push(spec('a', 'page'));

    dispatch({ key: 'q', code: 'KeyQ' });
    expect(stack.isEmpty()).toBe(true);
  });

  it("'handled' consumes without popping", () => {
    const onDismiss = vi.fn(() => 'handled' as const);
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onUserDismissRequest: onDismiss }));

    const e = dispatch({ key: 'Escape', code: 'Escape' });
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(stack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
  });

  it("'veto' consumes without popping", () => {
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onUserDismissRequest: () => 'veto' }));

    const e = dispatch({ key: 'q', code: 'KeyQ' });
    expect(stack.depth()).toBe(1);
    expect(e.defaultPrevented).toBe(true);
  });

  it('passes the key through unconsumed when an editable element inside the top is focused', () => {
    const onDismiss = vi.fn(() => 'close' as const);
    const { stack } = makeStack();
    const input = document.createElement('input');
    const page = stack.push(spec('a', 'page', { onUserDismissRequest: onDismiss }));
    page.element.appendChild(input);
    input.focus();
    const after = listenerAfterStack();

    const e = dispatch({ key: 'q', code: 'KeyQ' });
    expect(onDismiss).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(false);
    expect(after).toHaveBeenCalledOnce();
  });

  it('does not let an editable element in a lower layer shield the top', () => {
    const onDismiss = vi.fn(() => 'close' as const);
    const { stack } = makeStack();
    const lower = stack.push(spec('a', 'page'));
    const input = document.createElement('input');
    lower.element.appendChild(input);
    stack.push(spec('b', 'page', { onUserDismissRequest: onDismiss }));

    // Focus escapes to an input owned by the lower page while the top page is up.
    input.focus();
    dispatch({ key: 'Escape', code: 'Escape' });

    expect(onDismiss).toHaveBeenCalledOnce();
    expect(stack.find('b')).toBeNull();
  });

  it('a modal barrier blocks the global handler', () => {
    const globalHandler = vi.fn(() => true);
    const { stack } = makeStack();
    stack.setGlobalHandler(globalHandler);
    stack.push(spec('confirm', 'modal'));

    dispatch({ key: 'a', code: 'KeyA' });
    expect(globalHandler).not.toHaveBeenCalled();
  });

  it('a page barrier admits F1 and font combos to the global handler, and blocks everything else', () => {
    const globalHandler = vi.fn(() => true);
    const { stack } = makeStack();
    stack.setGlobalHandler(globalHandler);
    stack.push(spec('ws-hub', 'page'));

    dispatch({ key: 'F1', code: 'F1' });
    dispatch({ key: '1', code: 'Digit1', ctrlKey: true });
    expect(globalHandler).toHaveBeenCalledTimes(2);

    globalHandler.mockClear();
    dispatch({ key: 'S', code: 'KeyS', ctrlKey: true, shiftKey: true }); // new session
    dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true }); // tab switch
    expect(globalHandler).not.toHaveBeenCalled();
  });

  it('admits every font-size combo through a page barrier (parity with KeyboardShortcuts)', () => {
    const globalHandler = vi.fn(() => true);
    const { stack } = makeStack();
    stack.setGlobalHandler(globalHandler);
    stack.push(spec('ws-hub', 'page'));

    const fontCombos: KeyboardEventInit[] = [
      { key: '+', code: 'Equal', ctrlKey: true, shiftKey: true },
      { key: '+', code: 'NumpadAdd', ctrlKey: true, shiftKey: true },
      { key: '_', code: 'Minus', ctrlKey: true, shiftKey: true },
      { key: '_', code: 'NumpadSubtract', ctrlKey: true, shiftKey: true },
      { key: '=', code: 'Equal', ctrlKey: true },
      { key: '1', code: 'Digit1', ctrlKey: true },
      { key: '2', code: 'Digit2', ctrlKey: true },
      { key: '3', code: 'Digit3', ctrlKey: true },
    ];
    for (const combo of fontCombos) dispatch(combo);
    expect(globalHandler).toHaveBeenCalledTimes(fontCombos.length);
  });

  it('lets keys fall through a panel to the global handler', () => {
    const globalHandler = vi.fn(() => true);
    const { stack } = makeStack();
    stack.setGlobalHandler(globalHandler);
    const host = document.createElement('div');
    document.body.appendChild(host);
    stack.push(spec('crit', 'panel', {}, host));

    const e = dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });
    expect(globalHandler).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('Ctrl+S is a dismiss key for a page but not for a modal', () => {
    const pageDismiss = vi.fn(() => 'close' as const);
    const { stack } = makeStack();
    stack.push(spec('ws-hub', 'page', { onUserDismissRequest: pageDismiss }));
    dispatch({ key: 's', code: 'KeyS', ctrlKey: true });
    expect(pageDismiss).toHaveBeenCalledOnce();

    const modalDismiss = vi.fn(() => 'close' as const);
    stack.push(spec('confirm', 'modal', { onUserDismissRequest: modalDismiss }));
    dispatch({ key: 's', code: 'KeyS', ctrlKey: true });
    expect(modalDismiss).not.toHaveBeenCalled();
  });

  it('stops immediate propagation on consumed keys', () => {
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onUserDismissRequest: () => 'close' }));
    const after = listenerAfterStack();

    dispatch({ key: 'Escape', code: 'Escape' });
    expect(after).not.toHaveBeenCalled();
  });

  it('routes to the global handler when the stack is empty', () => {
    const globalHandler = vi.fn(() => true);
    const { stack } = makeStack();
    stack.setGlobalHandler(globalHandler);

    const e = dispatch({ key: 's', code: 'KeyS', ctrlKey: true });
    expect(globalHandler).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it('always runs the key observer, even on consumed keys', () => {
    const observer = vi.fn();
    const { stack } = makeStack();
    stack.setKeyObserver(observer);
    stack.push(spec('a', 'page', { onUserDismissRequest: () => 'close' }));

    dispatch({ key: 'Escape', code: 'Escape' });
    expect(observer).toHaveBeenCalledOnce();
  });
});

describe('LayerStack — requestUserDismiss', () => {
  it('returns false when the stack is empty', () => {
    const { stack } = makeStack();
    expect(stack.requestUserDismiss()).toBe(false);
  });

  it('routes through the top layer onUserDismissRequest and pops on close', () => {
    const onDismiss = vi.fn(() => 'close' as const);
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onUserDismissRequest: onDismiss }));

    expect(stack.requestUserDismiss()).toBe(true);
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(stack.isEmpty()).toBe(true);
  });

  it('consumes without popping when the top vetoes', () => {
    const { stack } = makeStack();
    stack.push(spec('a', 'page', { onUserDismissRequest: () => 'veto' }));

    expect(stack.requestUserDismiss()).toBe(true);
    expect(stack.depth()).toBe(1);
  });
});
