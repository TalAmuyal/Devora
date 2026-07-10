import { describe, it, expect, afterEach } from 'vitest';
import { collectFocusables, cycleFocus } from '../tabTrap';

let root: HTMLElement;

function makeRoot(html: string): HTMLElement {
  root = document.createElement('div');
  root.tabIndex = -1;
  root.innerHTML = html;
  document.body.appendChild(root);
  return root;
}

afterEach(() => {
  document.body.innerHTML = '';
});

describe('collectFocusables', () => {
  it('returns the tabbable descendants in DOM order', () => {
    makeRoot(`
      <button id="a">A</button>
      <input id="b" />
      <a id="c" href="#">C</a>
      <select id="d"></select>
      <textarea id="e"></textarea>
    `);
    expect(collectFocusables(root).map((el) => el.id)).toEqual(['a', 'b', 'c', 'd', 'e']);
  });

  it('excludes disabled controls', () => {
    makeRoot(`<button id="a">A</button><button id="b" disabled>B</button>`);
    expect(collectFocusables(root).map((el) => el.id)).toEqual(['a']);
  });

  it('excludes elements taken out of the tab order (tabindex < 0)', () => {
    makeRoot(`<button id="a">A</button><div id="b" tabindex="-1">B</div><div id="c" tabindex="0">C</div>`);
    expect(collectFocusables(root).map((el) => el.id)).toEqual(['a', 'c']);
  });

  it('excludes hidden elements (hidden attribute, inline display:none, ancestor display:none)', () => {
    makeRoot(`
      <button id="a">A</button>
      <button id="b" hidden>B</button>
      <button id="c" style="display:none">C</button>
      <div style="display:none"><button id="d">D</button></div>
    `);
    expect(collectFocusables(root).map((el) => el.id)).toEqual(['a']);
  });
});

describe('cycleFocus', () => {
  function threeButtons(): HTMLElement[] {
    makeRoot(`<button id="a">A</button><button id="b">B</button><button id="c">C</button>`);
    return collectFocusables(root);
  }

  it('moves forward to the next focusable', () => {
    const [a] = threeButtons();
    a.focus();
    cycleFocus(root, false);
    expect(document.activeElement?.id).toBe('b');
  });

  it('wraps from the last focusable to the first when moving forward', () => {
    const [, , c] = threeButtons();
    c.focus();
    cycleFocus(root, false);
    expect(document.activeElement?.id).toBe('a');
  });

  it('wraps from the first focusable to the last when moving backward', () => {
    const [a] = threeButtons();
    a.focus();
    cycleFocus(root, true);
    expect(document.activeElement?.id).toBe('c');
  });

  it('recovers to the first focusable when focus escaped the trap', () => {
    threeButtons();
    const outside = document.createElement('button');
    document.body.appendChild(outside);
    outside.focus();
    cycleFocus(root, false);
    expect(document.activeElement?.id).toBe('a');
  });

  it('recovers to the last focusable when moving backward with focus escaped', () => {
    threeButtons();
    const outside = document.createElement('button');
    document.body.appendChild(outside);
    outside.focus();
    cycleFocus(root, true);
    expect(document.activeElement?.id).toBe('c');
  });

  it('focuses the root itself when there are no focusables', () => {
    makeRoot(`<div>no focusables here</div>`);
    cycleFocus(root, false);
    expect(document.activeElement).toBe(root);
  });
});
