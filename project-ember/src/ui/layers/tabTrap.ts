/** Focus-trap helpers for modal layers (ADR-003). */

const FOCUSABLE_SELECTOR = 'button, [href], input, select, textarea, [tabindex]';

/** Tabbable elements inside `root`, in DOM order. Excludes disabled controls, elements taken out of the tab order (`tabindex < 0`), and hidden elements. */
export function collectFocusables(root: HTMLElement): HTMLElement[] {
  const candidates = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
  return candidates.filter(
    (el) => !el.hasAttribute('disabled') && el.tabIndex >= 0 && !isHidden(el, root),
  );
}

/**
 * Move focus to the next (or previous, when `backwards`) focusable inside `root`, wrapping at the ends.
 * When focus currently sits outside `root` — it escaped — recover to the first (or last) focusable.
 * Focuses `root` itself when it holds no focusables.
 */
export function cycleFocus(root: HTMLElement, backwards: boolean): void {
  const focusables = collectFocusables(root);
  if (focusables.length === 0) {
    root.focus();
    return;
  }
  const active = document.activeElement;
  const current = active instanceof HTMLElement ? focusables.indexOf(active) : -1;
  const next =
    current === -1
      ? backwards
        ? focusables.length - 1
        : 0
      : (current + (backwards ? -1 : 1) + focusables.length) % focusables.length;
  focusables[next].focus();
}

/**
 * Hidden via the `hidden` attribute or an inline `display:none` / `visibility:hidden` on the element or any ancestor up to `root`.
 * Layout-based visibility is intentionally not consulted, so the check stays deterministic outside a real browser (happy-dom reports no layout).
 */
function isHidden(el: HTMLElement, root: HTMLElement): boolean {
  let node: HTMLElement | null = el;
  while (node) {
    if (node.hidden || node.style.display === 'none' || node.style.visibility === 'hidden') {
      return true;
    }
    if (node === root) break;
    node = node.parentElement;
  }
  return false;
}
