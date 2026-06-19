import { describe, it, expect, vi } from 'vitest';
import { createListCursor } from '../listCursor';

function makeItems(count: number): HTMLElement[] {
  return Array.from({ length: count }, () => document.createElement('div'));
}

function activeIndexes(items: HTMLElement[], cls: string): number[] {
  return items.flatMap((el, i) => (el.classList.contains(cls) ? [i] : []));
}

describe('createListCursor', () => {
  it('highlights the first item on the initial sync', () => {
    const items = makeItems(3);
    const cursor = createListCursor({ items: () => items, activeClass: 'active' });
    cursor.sync();
    expect(cursor.index()).toBe(0);
    expect(activeIndexes(items, 'active')).toEqual([0]);
    expect(cursor.active()).toBe(items[0]);
  });

  it('moves and clamps at both ends', () => {
    const items = makeItems(3);
    const cursor = createListCursor({ items: () => items, activeClass: 'active' });
    cursor.sync();

    cursor.move(1);
    expect(cursor.index()).toBe(1);
    cursor.move(5); // clamp at last
    expect(cursor.index()).toBe(2);
    expect(activeIndexes(items, 'active')).toEqual([2]);
    cursor.move(-9); // clamp at first
    expect(cursor.index()).toBe(0);
    expect(activeIndexes(items, 'active')).toEqual([0]);
  });

  it('set highlights exactly one item', () => {
    const items = makeItems(4);
    const cursor = createListCursor({ items: () => items, activeClass: 'active' });
    cursor.set(2);
    expect(cursor.index()).toBe(2);
    expect(activeIndexes(items, 'active')).toEqual([2]);
  });

  it('sync re-clamps the index when the item set shrinks', () => {
    let items = makeItems(5);
    const cursor = createListCursor({ items: () => items, activeClass: 'active' });
    cursor.set(4);
    expect(cursor.index()).toBe(4);

    items = items.slice(0, 2); // list shrank to 2 items
    cursor.sync();
    expect(cursor.index()).toBe(1);
    expect(activeIndexes(items, 'active')).toEqual([1]);
  });

  it('calls onChange with the active element and index', () => {
    const items = makeItems(3);
    const onChange = vi.fn();
    const cursor = createListCursor({ items: () => items, activeClass: 'active', onChange });
    cursor.set(1);
    expect(onChange).toHaveBeenLastCalledWith(items[1], 1);
  });

  it('is a no-op on move with an empty list, and reports a null active', () => {
    const items: HTMLElement[] = [];
    const onChange = vi.fn();
    const cursor = createListCursor({ items: () => items, activeClass: 'active', onChange });

    expect(() => cursor.move(1)).not.toThrow();
    expect(cursor.active()).toBeNull();

    cursor.sync();
    expect(onChange).toHaveBeenLastCalledWith(null, 0);
  });
});
