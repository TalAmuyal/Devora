import { describe, it, expect, vi } from 'vitest';
import { pagerScrollDelta, createVimScroll } from '../vimScroll';

function key(init: KeyboardEventInit): KeyboardEvent {
  return new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
}

function fakeContainer(clientHeight = 1000, scrollHeight = 5000) {
  return {
    clientHeight,
    scrollHeight,
    scrollTop: 0,
    scrollBy: vi.fn(),
  };
}

type FakeContainer = ReturnType<typeof fakeContainer>;
const asContainer = (c: FakeContainer): HTMLElement => c as unknown as HTMLElement;

describe('pagerScrollDelta', () => {
  const H = 1000;

  it('maps j / ArrowDown to a downward line nudge', () => {
    expect(pagerScrollDelta(key({ key: 'j' }), H)).toBe(100);
    expect(pagerScrollDelta(key({ key: 'ArrowDown' }), H)).toBe(100);
  });

  it('maps k / ArrowUp to an upward line nudge', () => {
    expect(pagerScrollDelta(key({ key: 'k' }), H)).toBe(-100);
    expect(pagerScrollDelta(key({ key: 'ArrowUp' }), H)).toBe(-100);
  });

  it('maps Ctrl+D / Ctrl+U to half-page jumps', () => {
    expect(pagerScrollDelta(key({ key: 'd', code: 'KeyD', ctrlKey: true }), H)).toBe(500);
    expect(pagerScrollDelta(key({ key: 'u', code: 'KeyU', ctrlKey: true }), H)).toBe(-500);
  });

  it('ignores non-pager keys and wrong modifiers', () => {
    expect(pagerScrollDelta(key({ key: 'd', code: 'KeyD', ctrlKey: true, shiftKey: true }), H)).toBeNull();
    expect(pagerScrollDelta(key({ key: 'd', code: 'KeyD' }), H)).toBeNull();
    expect(pagerScrollDelta(key({ key: 'j', ctrlKey: true }), H)).toBeNull();
    expect(pagerScrollDelta(key({ key: 'x' }), H)).toBeNull();
  });
});

describe('createVimScroll', () => {
  it('jumps to the bottom on G', () => {
    const vs = createVimScroll();
    const c = fakeContainer();
    expect(vs.handleKey(key({ key: 'G' }), asContainer(c))).toBe(true);
    expect(c.scrollTop).toBe(5000);
  });

  it('jumps to the top on gg', () => {
    const vs = createVimScroll();
    const c = fakeContainer();
    c.scrollTop = 999;
    expect(vs.handleKey(key({ key: 'g' }), asContainer(c))).toBe(true); // first g: armed, no scroll
    expect(c.scrollTop).toBe(999);
    expect(vs.handleKey(key({ key: 'g' }), asContainer(c))).toBe(true);
    expect(c.scrollTop).toBe(0);
  });

  it('lets a non-g key after g behave normally and re-arms afterwards', () => {
    const vs = createVimScroll();
    const c = fakeContainer();
    c.scrollTop = 500;
    vs.handleKey(key({ key: 'g' }), asContainer(c)); // arm
    expect(vs.handleKey(key({ key: 'j' }), asContainer(c))).toBe(true); // j scrolls, pending cleared
    expect(c.scrollBy).toHaveBeenCalledWith({ top: 100, behavior: 'smooth' });
    // A following lone g arms again — it must not be treated as the second g of a gg (which would jump to top).
    expect(vs.handleKey(key({ key: 'g' }), asContainer(c))).toBe(true);
    expect(c.scrollTop).toBe(500); // not jumped to top
  });

  it('does not consume an unrelated key after g', () => {
    const vs = createVimScroll();
    const c = fakeContainer();
    vs.handleKey(key({ key: 'g' }), asContainer(c));
    expect(vs.handleKey(key({ key: 'x' }), asContainer(c))).toBe(false);
  });

  it('reset clears a pending g', () => {
    const vs = createVimScroll();
    const c = fakeContainer();
    c.scrollTop = 500;
    vs.handleKey(key({ key: 'g' }), asContainer(c)); // arm
    vs.reset();
    expect(vs.handleKey(key({ key: 'g' }), asContainer(c))).toBe(true); // armed again, not a jump
    expect(c.scrollTop).toBe(500); // not jumped to top
  });

  it('scrolls half a page on Ctrl+D', () => {
    const vs = createVimScroll();
    const c = fakeContainer();
    expect(vs.handleKey(key({ key: 'd', code: 'KeyD', ctrlKey: true }), asContainer(c))).toBe(true);
    expect(c.scrollBy).toHaveBeenCalledWith({ top: 500, behavior: 'smooth' });
  });
});
