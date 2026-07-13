import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { LayerStack } from '../layers/LayerStack';
import { SessionPanels } from '../SessionPanels';
import type { LayerSpec } from '../layers/types';

/**
 * SessionPanels drives the per-session panel overlays through a real LayerStack (ADR-003).
 * These tests exercise the sentinel model: content stays mounted across tab switches (no iframe reload), the active panel is the bottom layer (focused only when top), q routes to the panel's own dismiss decision, and a hidden panel's removal never steals focus.
 */

let stack: LayerStack;
let mainPanelEl: HTMLElement;
let activeId: number | null;
let baseFocus: { focus: ReturnType<typeof vi.fn> };
let onChange: ReturnType<typeof vi.fn>;
let panels: SessionPanels;

function makePanels(): SessionPanels {
  return new SessionPanels({
    layers: stack,
    mainPanelEl,
    getActiveSessionId: () => activeId,
    onChange,
  });
}

/** Activate a session as SessionManager would: flip the active id, then notify. */
function activate(id: number | null): void {
  activeId = id;
  panels.syncActive();
}

function content(tag: string): HTMLElement {
  const el = document.createElement('div');
  el.dataset.tag = tag;
  return el;
}

function pushPage(onKey?: (e: KeyboardEvent) => boolean): LayerSpec {
  const spec: LayerSpec = {
    name: 'a-page',
    kind: 'page',
    element: document.createElement('div'),
    onKey,
    onUserDismissRequest: () => 'close',
  };
  stack.push(spec);
  return spec;
}

function dispatch(init: KeyboardEventInit): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(e);
  return e;
}

beforeEach(() => {
  const pageHost = document.createElement('div');
  document.body.appendChild(pageHost);
  mainPanelEl = document.createElement('div');
  document.body.appendChild(mainPanelEl);
  baseFocus = { focus: vi.fn() };
  stack = new LayerStack({
    pageHost,
    modalHost: document.body,
    resolveBaseFocus: () => baseFocus,
  });
  stack.install();
  activeId = null;
  onChange = vi.fn();
  panels = makePanels();
});

afterEach(() => {
  stack.clear();
  stack.uninstall();
  document.body.innerHTML = '';
});

describe('SessionPanels visibility on activation', () => {
  it('shows the active session panel and hides the others', () => {
    activeId = 1;
    const c1 = content('one');
    const c2 = content('two');
    panels.show(1, c1, () => 'close');
    panels.show(2, c2, () => 'close');

    expect((c1.parentElement as HTMLElement).style.display).toBe('block');
    expect((c2.parentElement as HTMLElement).style.display).toBe('none');

    activate(2);
    expect((c1.parentElement as HTMLElement).style.display).toBe('none');
    expect((c2.parentElement as HTMLElement).style.display).toBe('block');
  });

  it('keeps the content mounted across a switch away and back (no reload)', () => {
    activeId = 1;
    const c1 = content('one');
    panels.show(1, c1, () => 'close');
    const wrapper = c1.parentElement!;
    expect(mainPanelEl.contains(wrapper)).toBe(true);

    activate(2);
    expect(mainPanelEl.contains(wrapper)).toBe(true);
    expect(c1.parentElement).toBe(wrapper);

    activate(1);
    expect(mainPanelEl.contains(wrapper)).toBe(true);
    expect((wrapper as HTMLElement).style.display).toBe('block');
  });
});

describe('SessionPanels as a stack layer', () => {
  it("makes the active session's panel the bottom layer, focused only when it is on top", () => {
    activeId = 1;
    panels.show(1, content('one'), () => 'close');

    const sentinel = mainPanelEl.querySelector('.session-panel-sentinel') as HTMLElement;
    expect(stack.topOf('panel')).not.toBeNull();
    expect(document.activeElement).toBe(sentinel); // sole layer → focused

    pushPage();
    expect(stack.topOf('panel')).not.toBeNull(); // still on the stack, at the bottom
    expect(document.activeElement).not.toBe(sentinel); // no longer top → not focused
  });

  it('refocuses the active panel when switching between two panelled sessions', () => {
    activeId = 1;
    panels.show(1, content('one'), () => 'close');
    panels.show(2, content('two'), () => 'close');

    activate(2);
    const sentinel = mainPanelEl.querySelector('.session-panel-sentinel');
    expect(document.activeElement).toBe(sentinel);
  });

  it('lets Ctrl+ArrowRight pass through the panel to the base shortcuts', () => {
    const globalHandler = vi.fn(() => true);
    stack.setGlobalHandler(globalHandler);
    const dismiss = vi.fn((): 'close' => 'close');
    activeId = 1;
    panels.show(1, content('one'), dismiss);

    const e = dispatch({ key: 'ArrowRight', code: 'ArrowRight', ctrlKey: true });

    expect(globalHandler).toHaveBeenCalledOnce();
    expect(dismiss).not.toHaveBeenCalled();
    expect(e.defaultPrevented).toBe(true);
  });

  it('leaves a covering page in charge of routing', () => {
    activeId = 1;
    const dismiss = vi.fn((): 'close' => 'close');
    panels.show(1, content('one'), dismiss);
    pushPage();

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(stack.find('a-page')).toBeNull(); // the page took the q and closed
    expect(dismiss).not.toHaveBeenCalled(); // the panel beneath was untouched
    expect(panels.has(1)).toBe(true);
  });
});

describe('SessionPanels dismissal', () => {
  it('dismisses the visible panel on q, consulting its callback and restoring base focus', () => {
    activeId = 1;
    const c1 = content('one');
    const dismiss = vi.fn((): 'close' => 'close');
    panels.show(1, c1, dismiss);

    dispatch({ key: 'q', code: 'KeyQ' });

    expect(dismiss).toHaveBeenCalledOnce();
    expect(panels.has(1)).toBe(false);
    expect(mainPanelEl.contains(c1)).toBe(false);
    expect(stack.topOf('panel')).toBeNull();
    expect(baseFocus.focus).toHaveBeenCalledOnce(); // stack emptied → focus the terminal
  });

  it("keeps the panel when its callback returns 'handled' (controller-owned teardown)", () => {
    activeId = 1;
    const dismiss = vi.fn((): 'handled' => 'handled');
    panels.show(1, content('one'), dismiss);

    const e = dispatch({ key: 'q', code: 'KeyQ' });

    expect(dismiss).toHaveBeenCalledOnce();
    expect(panels.has(1)).toBe(true); // still shown; the controller will tear it down later
    expect(stack.topOf('panel')).not.toBeNull();
    expect(e.defaultPrevented).toBe(true); // the key is consumed either way
  });

  it('does not steal focus when a hidden panel is dismissed by the backend', () => {
    activeId = 2;
    panels.show(2, content('two'), () => 'close'); // active session's panel — holds the sentinel/focus
    panels.show(1, content('one'), () => 'close'); // background session's panel — hidden, off the stack
    const sentinel = mainPanelEl.querySelector('.session-panel-sentinel');
    expect(document.activeElement).toBe(sentinel);
    baseFocus.focus.mockClear();

    panels.dismiss(1);

    expect(panels.has(1)).toBe(false);
    expect(baseFocus.focus).not.toHaveBeenCalled(); // no focus moved
    expect(document.activeElement).toBe(sentinel); // the active panel keeps focus
    expect(stack.topOf('panel')).not.toBeNull();
  });

  it('re-renders the tab bar exactly on show and on dismiss', () => {
    activeId = 1;
    panels.show(1, content('one'), () => 'close');
    expect(onChange).toHaveBeenCalledTimes(1);

    panels.dismiss(1);
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(panels.has(1)).toBe(false);
  });
});
