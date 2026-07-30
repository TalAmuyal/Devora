import { describe, it, expect, vi, afterEach } from 'vitest';
import { LayerStack, LayerStackDeps } from '../LayerStack';
import { modalLayer, pageLayer, popupLayer, terminalLayer } from '../presets';
import type { Focusable, LayerSpec } from '../types';

/**
 * The stack mechanism in isolation: mounting, focus, reveal, and what `route` reports.
 * The stack owns no keyboard listener — `LayerRouter.test.ts` covers the listener and event consumption,
 * and `pageRouting.test.ts` covers the concrete surfaces `main.ts` wires.
 */

interface Built {
  stack: LayerStack;
  host: HTMLElement;
}

function makeStack(deps: Partial<LayerStackDeps> = {}): Built {
  const host = document.createElement('div');
  host.className = 'layer-host';
  document.body.appendChild(host);
  const stack = new LayerStack({ host, ...deps });
  return { stack, host };
}

function content(name: string, place?: HTMLElement): HTMLElement {
  const el = document.createElement('div');
  el.className = `${name}-content`;
  if (place) place.appendChild(el);
  return el;
}

function keydown(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
}

function focusableSpy(): Focusable & { focus: ReturnType<typeof vi.fn> } {
  return { focus: vi.fn() };
}

/** A bare layer with every default in force — the baseline the presets vary from. */
function bare(name: string, over: Partial<LayerSpec> = {}): LayerSpec {
  return { name, element: content(name), ...over };
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('LayerStack — stack operations', () => {
  it('pushes onto the top and reports depth/top/isEmpty/find', () => {
    const { stack } = makeStack();
    expect(stack.isEmpty()).toBe(true);

    const a = stack.push(pageLayer({ name: 'a', element: content('a') }));
    const b = stack.push(pageLayer({ name: 'b', element: content('b') }));

    expect(stack.depth()).toBe(2);
    expect(stack.isEmpty()).toBe(false);
    expect(stack.top()).toBe(b);
    expect(stack.find('a')).toBe(a);
  });

  it('appends every stack-owned wrapper to the one host, in stack order', () => {
    const { stack, host } = makeStack();
    const page = stack.push(pageLayer({ name: 'ws-hub', element: content('ws-hub') }));
    const modal = stack.push(modalLayer({ name: 'confirm', element: content('confirm') }));

    // Paint order is DOM order, so the later push must be the later child — this is the whole z-index story.
    expect(Array.from(host.children)).toEqual([page.wrapper, modal.wrapper]);
    expect(page.wrapper.classList.contains('layer-wrapper')).toBe(true);
    expect(page.wrapper.classList.contains('layer-page')).toBe(true);
    expect(page.wrapper.contains(page.element)).toBe(true);
    expect(modal.wrapper.classList.contains('layer-modal')).toBe(true);
    expect(modal.wrapper.classList.contains('confirm-backdrop')).toBe(true);
  });

  it('leaves a caller-mounted layer where the caller put it', () => {
    const { stack, host } = makeStack();
    const anchor = document.createElement('div');
    document.body.appendChild(anchor);

    const popup = stack.push(popupLayer({ name: 'menu', element: content('menu', anchor) }));

    expect(popup.wrapper).toBe(popup.element);
    expect(popup.wrapper.parentElement).toBe(anchor);
    expect(host.children.length).toBe(0);
  });

  it('adds wrapperClass alongside the preset classes', () => {
    const { stack } = makeStack();
    const page = stack.push(
      pageLayer({ name: 'palette', element: content('palette'), wrapperClass: 'layer-transparent' }),
    );
    expect(page.wrapper.classList.contains('layer-page')).toBe(true);
    expect(page.wrapper.classList.contains('layer-transparent')).toBe(true);
  });

  it('pops the top and detaches its wrapper', () => {
    const { stack } = makeStack();
    const a = stack.push(pageLayer({ name: 'a', element: content('a') }));
    stack.push(pageLayer({ name: 'b', element: content('b') }));

    stack.pop();
    expect(stack.depth()).toBe(1);
    expect(stack.top()).toBe(a);
    expect(stack.find('b')).toBeNull();
  });

  it('removing a non-top layer detaches it without revealing or refocusing', () => {
    const onEmptied = vi.fn();
    const { stack } = makeStack({ onEmptied });
    const a = stack.push(pageLayer({ name: 'a', element: content('a'), onReveal: vi.fn() }));
    const b = stack.push(pageLayer({ name: 'b', element: content('b') }));

    stack.remove(a);

    expect(stack.depth()).toBe(1);
    expect(stack.top()).toBe(b);
    expect(a.wrapper.parentElement).toBeNull();
    expect(onEmptied).not.toHaveBeenCalled();
  });

  it('removing a page auto-removes a popup anchored inside it and reports the stack empty', () => {
    const onEmptied = vi.fn();
    const { stack } = makeStack({ onEmptied });
    const page = stack.push(pageLayer({ name: 'ws-hub', element: content('ws-hub') }));
    const popupCleanup = vi.fn();
    // The dropdown lives inside the page content, so it is DOM-contained in the page wrapper.
    const popup = stack.push(
      popupLayer({
        name: 'dropdown',
        element: content('dropdown', page.element),
        onCleanup: popupCleanup,
      }),
    );

    stack.remove(page);

    expect(stack.isEmpty()).toBe(true);
    expect(popupCleanup).toHaveBeenCalledOnce();
    expect(popup.wrapper.parentElement).toBeNull();
    expect(onEmptied).toHaveBeenCalledOnce();
  });

  it('reports whether any layer owns the keyboard', () => {
    const { stack } = makeStack();
    expect(stack.hasOpaqueLayer()).toBe(false);

    const anchor = document.createElement('div');
    document.body.appendChild(anchor);
    const popup = stack.push(popupLayer({ name: 'menu', element: content('menu', anchor) }));
    // A dropdown does not own the keyboard, so it must not gate the shortcuts that check this.
    expect(stack.hasOpaqueLayer()).toBe(false);

    stack.push(pageLayer({ name: 'ws-hub', element: content('ws-hub') }));
    expect(stack.hasOpaqueLayer()).toBe(true);
    void popup;
  });
});

describe('LayerStack — reveal & cleanup', () => {
  it('runs onReveal on the revealed layer when a page pops', () => {
    const onReveal = vi.fn();
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a'), onReveal }));
    stack.push(pageLayer({ name: 'b', element: content('b') }));

    stack.pop();
    expect(onReveal).toHaveBeenCalledOnce();
  });

  it('restores focus to the revealed page on a modal pop but does not fire its onReveal', () => {
    const onReveal = vi.fn();
    const focus = focusableSpy();
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'page', element: content('page'), onReveal, resolveFocus: () => focus }));
    stack.push(modalLayer({ name: 'confirm', element: content('confirm') }));

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
    stack.push(pageLayer({ name: 'a', element: content('a'), onReveal: onRevealA }));
    stack.push(pageLayer({ name: 'b', element: content('b'), onCleanup: cleanupB }));

    const c = stack.replaceTop(pageLayer({ name: 'c', element: content('c') }));

    expect(cleanupB).toHaveBeenCalledOnce();
    expect(onRevealA).not.toHaveBeenCalled();
    expect(stack.depth()).toBe(2);
    expect(stack.top()).toBe(c);
    expect(stack.find('a')).not.toBeNull();
  });

  it('closing a popup over a page leaves the page undisturbed (no onReveal, no focus move)', () => {
    const onReveal = vi.fn();
    const { stack } = makeStack();
    const page = stack.push(pageLayer({ name: 'ws-hub', element: content('ws-hub'), onReveal }));
    const focusedInPage = document.createElement('button');
    page.element.appendChild(focusedInPage);
    focusedInPage.focus();
    const popup = stack.push(popupLayer({ name: 'dropdown', element: content('dropdown', page.element) }));

    stack.remove(popup);

    expect(onReveal).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(focusedInPage);
    expect(stack.top()).toBe(page);
  });

  it('runs cleanup exactly once across pop', () => {
    const onCleanup = vi.fn();
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a'), onCleanup }));

    stack.pop();
    stack.pop(); // no-op: nothing left
    expect(onCleanup).toHaveBeenCalledOnce();
  });

  it('clear() cleans up and detaches every layer once', () => {
    const cleanupA = vi.fn();
    const cleanupB = vi.fn();
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a'), onCleanup: cleanupA }));
    stack.push(modalLayer({ name: 'b', element: content('b'), onCleanup: cleanupB }));

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
    stack.push(pageLayer({ name: 'a', element: content('a'), resolveFocus: () => target }));
    expect(target.focus).toHaveBeenCalledOnce();
  });

  it('focuses the wrapper on push when resolveFocus is absent', () => {
    const { stack } = makeStack();
    const a = stack.push(pageLayer({ name: 'a', element: content('a') }));
    expect(document.activeElement).toBe(a.wrapper);
  });

  it('does not move focus when a popup is pushed', () => {
    const { stack } = makeStack();
    const sentinel = document.createElement('button');
    document.body.appendChild(sentinel);
    sentinel.focus();

    const anchor = document.createElement('div');
    document.body.appendChild(anchor);
    stack.push(popupLayer({ name: 'menu', element: content('menu', anchor) }));

    expect(document.activeElement).toBe(sentinel);
  });

  it('focuses the revealed layer when the top pops', () => {
    const { stack } = makeStack();
    const a = stack.push(pageLayer({ name: 'a', element: content('a') }));
    stack.push(pageLayer({ name: 'b', element: content('b') }));

    stack.pop();
    expect(document.activeElement).toBe(a.wrapper);
  });

  it('focusTop re-focuses the current top (tab activation)', () => {
    const { stack } = makeStack();
    const target = focusableSpy();
    stack.push(pageLayer({ name: 'a', element: content('a'), resolveFocus: () => target }));
    target.focus.mockClear();

    stack.focusTop();
    expect(target.focus).toHaveBeenCalledOnce();
  });

  it('reports emptiness at pop time, so a late-changing owner still gets focus', () => {
    let notified = 0;
    const { stack } = makeStack({ onEmptied: () => (notified += 1) });
    stack.push(pageLayer({ name: 'a', element: content('a') }));

    stack.pop();
    expect(notified).toBe(1);
  });
});

describe('LayerStack — route', () => {
  it('lets the top layer onKey win', () => {
    const { stack } = makeStack();
    const lowerKey = vi.fn(() => false);
    const topKey = vi.fn(() => true);
    stack.push(pageLayer({ name: 'a', element: content('a'), onKey: lowerKey }));
    stack.push(pageLayer({ name: 'b', element: content('b'), onKey: topKey }));

    expect(stack.route(keydown({ key: 'x', code: 'KeyX' }))).toBe('consumed');
    expect(topKey).toHaveBeenCalledOnce();
    expect(lowerKey).not.toHaveBeenCalled();
  });

  it('passes when the stack is empty', () => {
    const { stack } = makeStack();
    expect(stack.route(keydown({ key: 'x', code: 'KeyX' }))).toBe('passed');
  });

  it('traps Tab within a layer that asks for it', () => {
    const { stack } = makeStack();
    const element = content('confirm');
    const first = document.createElement('button');
    const second = document.createElement('button');
    first.id = 'first';
    second.id = 'second';
    element.append(first, second);
    stack.push(modalLayer({ name: 'confirm', element }));
    first.focus();

    expect(stack.route(keydown({ key: 'Tab', code: 'Tab' }))).toBe('consumed');
    expect(document.activeElement?.id).toBe('second');
  });

  it('does not trap Tab for a layer that does not ask for it', () => {
    const { stack } = makeStack();
    const element = content('ws-hub');
    const button = document.createElement('button');
    element.appendChild(button);
    stack.push(pageLayer({ name: 'ws-hub', element }));
    button.focus();

    expect(stack.route(keydown({ key: 'Tab', code: 'Tab' }))).toBe('blocked');
    expect(document.activeElement).toBe(button);
  });

  it('Escape and q dismiss the top, popping it on the default decision', () => {
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a') }));
    stack.push(pageLayer({ name: 'b', element: content('b') }));

    expect(stack.route(keydown({ key: 'Escape', code: 'Escape' }))).toBe('consumed');
    expect(stack.find('b')).toBeNull();

    expect(stack.route(keydown({ key: 'q', code: 'KeyQ' }))).toBe('consumed');
    expect(stack.isEmpty()).toBe(true);
  });

  it("'handled' and 'veto' consume without popping", () => {
    const { stack } = makeStack();
    const handled = vi.fn(() => 'handled' as const);
    stack.push(pageLayer({ name: 'a', element: content('a'), onUserDismissRequest: handled }));

    expect(stack.route(keydown({ key: 'Escape', code: 'Escape' }))).toBe('consumed');
    expect(handled).toHaveBeenCalledOnce();
    expect(stack.depth()).toBe(1);

    stack.push(pageLayer({ name: 'b', element: content('b'), onUserDismissRequest: () => 'veto' }));
    expect(stack.route(keydown({ key: 'q', code: 'KeyQ' }))).toBe('consumed');
    expect(stack.depth()).toBe(2);
  });

  it('blocks without dismissing when an editable element inside the top is focused', () => {
    const onDismiss = vi.fn(() => 'close' as const);
    const { stack } = makeStack();
    const element = content('a');
    const input = document.createElement('input');
    element.appendChild(input);
    stack.push(pageLayer({ name: 'a', element, onUserDismissRequest: onDismiss }));
    input.focus();

    expect(stack.route(keydown({ key: 'q', code: 'KeyQ' }))).toBe('blocked');
    expect(onDismiss).not.toHaveBeenCalled();
  });

  it('does not let an editable element in a lower layer shield the top', () => {
    const onDismiss = vi.fn(() => 'close' as const);
    const { stack } = makeStack();
    const lower = stack.push(pageLayer({ name: 'a', element: content('a') }));
    const input = document.createElement('input');
    lower.element.appendChild(input);
    stack.push(pageLayer({ name: 'b', element: content('b'), onUserDismissRequest: onDismiss }));

    // Focus escapes to an input owned by the lower page while the top page is up.
    input.focus();
    expect(stack.route(keydown({ key: 'Escape', code: 'Escape' }))).toBe('consumed');
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(stack.find('b')).toBeNull();
  });

  it('an opaque layer ends the walk without consuming', () => {
    const lowerKey = vi.fn(() => true);
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a'), onKey: lowerKey }));
    stack.push(bare('opaque'));

    expect(stack.route(keydown({ key: 'x', code: 'KeyX' }))).toBe('blocked');
    expect(lowerKey).not.toHaveBeenCalled();
  });

  it('a transparent layer passes unhandled keys down and then out of the stack', () => {
    const { stack } = makeStack();
    const anchor = document.createElement('div');
    document.body.appendChild(anchor);
    stack.push(popupLayer({ name: 'menu', element: content('menu', anchor) }));

    expect(stack.route(keydown({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true }))).toBe('passed');
  });

  it('a non-dismissible layer blocks q and Escape instead of closing on them', () => {
    const { stack } = makeStack();
    const terminal = stack.push(terminalLayer({ name: 'terminal', element: content('terminal') }));

    // A terminal must stay put and let the key reach xterm — closing on `q` is the trap this guards.
    expect(stack.route(keydown({ key: 'q', code: 'KeyQ' }))).toBe('blocked');
    expect(stack.route(keydown({ key: 'Escape', code: 'Escape' }))).toBe('blocked');
    expect(stack.top()).toBe(terminal);
  });

  it('treats a modified q or Escape as a shortcut, never a dismissal', () => {
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a') }));

    expect(stack.route(keydown({ key: 'q', code: 'KeyQ', ctrlKey: true }))).toBe('blocked');
    expect(stack.depth()).toBe(1);
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
    stack.push(pageLayer({ name: 'a', element: content('a'), onUserDismissRequest: onDismiss }));

    expect(stack.requestUserDismiss()).toBe(true);
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(stack.isEmpty()).toBe(true);
  });

  it('reports handled without popping when the top vetoes', () => {
    const { stack } = makeStack();
    stack.push(pageLayer({ name: 'a', element: content('a'), onUserDismissRequest: () => 'veto' }));

    expect(stack.requestUserDismiss()).toBe(true);
    expect(stack.depth()).toBe(1);
  });
});
