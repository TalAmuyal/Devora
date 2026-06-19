/**
 * A keyboard "active item" cursor over a dynamic list of elements: tracks a clamped index, applies a highlight class to the active element, and scrolls it into view.
 * Shared by the Command Palette and RepoList so the navigation kernel lives in one place.
 * Callers may freely hide or rebuild the underlying elements between operations — `items()` is re-read each time and `sync()` re-clamps.
 */

export interface ListCursorOptions {
  /** The currently navigable elements, in order. Re-read on every operation. */
  items: () => HTMLElement[];
  /** Class toggled on the active element (e.g. 'selected' or 'active'). */
  activeClass: string;
  /** Called after the active element changes; receives `null` when the list is empty. */
  onChange?: (active: HTMLElement | null, index: number) => void;
}

export interface ListCursorHandle {
  index(): number;
  active(): HTMLElement | null;
  /** Move the cursor by `delta`, clamped to the list; no-op on an empty list. */
  move(delta: number): void;
  /** Place the cursor at `index` (clamped). */
  set(index: number): void;
  /** Re-clamp the index into the current list and re-apply the highlight. Call after the list changes. */
  sync(): void;
}

export function createListCursor(options: ListCursorOptions): ListCursorHandle {
  let idx = 0;

  function apply(): void {
    const items = options.items();
    if (items.length === 0) {
      idx = 0;
      options.onChange?.(null, 0);
      return;
    }
    idx = Math.max(0, Math.min(idx, items.length - 1));
    items.forEach((el, i) => el.classList.toggle(options.activeClass, i === idx));
    items[idx].scrollIntoView?.({ block: 'nearest' });
    options.onChange?.(items[idx], idx);
  }

  return {
    index: () => idx,
    active: () => options.items()[idx] ?? null,
    move(delta) {
      const count = options.items().length;
      if (count === 0) return;
      idx = Math.max(0, Math.min(idx + delta, count - 1));
      apply();
    },
    set(index) {
      idx = index;
      apply();
    },
    sync() {
      apply();
    },
  };
}
