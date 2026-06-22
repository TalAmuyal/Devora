/**
 * A draggable vertical divider that resizes the preview pane to its right by adjusting that pane's flex-basis.
 * The terminal (flex:1) absorbs the difference; its xterm instance refits automatically via its ResizeObserver.
 * DOM: `div.preview-divider`.
 */
export interface PreviewDividerHandle {
  readonly el: HTMLElement;
  dispose(): void;
}

/** Smallest the resized preview pane may become. */
const MIN_PREVIEW_WIDTH = 200;
/** Smallest the terminal may become while dragging. */
const MIN_TERMINAL_WIDTH = 240;

export function createPreviewDivider(opts: {
  paneEl: HTMLElement;
  terminalEl: HTMLElement;
  splitEl: HTMLElement;
}): PreviewDividerHandle {
  const { paneEl, terminalEl, splitEl } = opts;

  const el = document.createElement('div');
  el.className = 'preview-divider';

  let dragging = false;

  // Width consumed by everything except the terminal (elastic) and the pane being dragged — i.e. other panes and all dividers.
  // Used to keep the terminal above its minimum.
  const otherFixedWidth = (): number => {
    let total = 0;
    for (const child of Array.from(splitEl.children)) {
      if (child === paneEl || child === terminalEl) continue;
      total += (child as HTMLElement).offsetWidth;
    }
    return total;
  };

  const onPointerDown = (e: PointerEvent) => {
    dragging = true;
    el.setPointerCapture(e.pointerId);
    e.preventDefault();
  };

  const onPointerMove = (e: PointerEvent) => {
    if (!dragging) return;
    const rect = splitEl.getBoundingClientRect();
    const maxWidth = splitEl.clientWidth - otherFixedWidth() - MIN_TERMINAL_WIDTH;
    const desired = rect.right - e.clientX;
    const clamped = Math.max(MIN_PREVIEW_WIDTH, Math.min(desired, maxWidth));
    paneEl.style.flexBasis = `${clamped}px`;
  };

  const onPointerUp = (e: PointerEvent) => {
    dragging = false;
    el.releasePointerCapture(e.pointerId);
  };

  el.addEventListener('pointerdown', onPointerDown);
  el.addEventListener('pointermove', onPointerMove);
  el.addEventListener('pointerup', onPointerUp);

  return {
    el,
    dispose() {
      el.removeEventListener('pointerdown', onPointerDown);
      el.removeEventListener('pointermove', onPointerMove);
      el.removeEventListener('pointerup', onPointerUp);
    },
  };
}
