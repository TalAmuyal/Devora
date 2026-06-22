import { describe, it, expect, vi } from 'vitest';
import { createPreviewDivider } from '../PreviewDivider';

function setup() {
  const splitEl = document.createElement('div');
  const terminalEl = document.createElement('div');
  const paneEl = document.createElement('div');
  splitEl.append(terminalEl, paneEl);
  const { el, dispose } = createPreviewDivider({ paneEl, terminalEl, splitEl });
  // happy-dom has no real layout/pointer capture; stub so the handlers run.
  el.setPointerCapture = vi.fn();
  el.releasePointerCapture = vi.fn();
  splitEl.insertBefore(el, paneEl);
  return { splitEl, terminalEl, paneEl, el, dispose };
}

function drag(el: HTMLElement, clientX: number) {
  el.dispatchEvent(new MouseEvent('pointerdown', { clientX: 0 }));
  el.dispatchEvent(new MouseEvent('pointermove', { clientX }));
  el.dispatchEvent(new MouseEvent('pointerup', { clientX }));
}

describe('createPreviewDivider', () => {
  it('builds a .preview-divider element', () => {
    const { el } = setup();
    expect(el.classList.contains('preview-divider')).toBe(true);
  });

  it('sets the pane flex-basis while dragging, clamped to the minimum width', () => {
    const { el, paneEl } = setup();
    drag(el, 9999); // far-right drag; with no layout, clamps to the floor
    expect(paneEl.style.flexBasis).toBe('200px');
  });

  it('stops adjusting the pane after dispose()', () => {
    const { el, paneEl, dispose } = setup();
    drag(el, 100);
    paneEl.style.flexBasis = '';
    dispose();
    drag(el, 100);
    expect(paneEl.style.flexBasis).toBe('');
  });
});
