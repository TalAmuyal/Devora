/**
 * Vim/Neovim-style keyboard scrolling for a scroll container (like the read-only surfaces).
 * See ADR-004.
 *
 * `gg` is a two-key sequence, so this is a small stateful controller (a factory + handle, like `createListCursor`) rather than a pure function; the relative-motion math is factored into the pure `pagerScrollDelta` so it can be unit-tested without layout.
 */

// Fraction of the visible height per j/k press: small enough to read line-by-line, viewport-relative so it tracks window/UI-scale changes rather than a fixed pixel count.
const LINE_FRACTION = 0.1;

export interface VimScroll {
  /** Handle a keydown against `container`; returns `true` when the key was a scroll motion (and was consumed). */
  handleKey(e: KeyboardEvent, container: HTMLElement): boolean;
  /** Drop pending multi-key state (`g…`); call when the surface hides/unloads. */
  reset(): void;
}

/** The relative scroll delta (px, positive = down) a pager key maps to for a viewport of `viewportHeight`, or `null` when `e` is not a relative-motion key. */
export function pagerScrollDelta(e: KeyboardEvent, viewportHeight: number): number | null {
  const noMods = !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey;
  if (noMods && (e.key === 'j' || e.key === 'ArrowDown')) return viewportHeight * LINE_FRACTION;
  if (noMods && (e.key === 'k' || e.key === 'ArrowUp')) return -viewportHeight * LINE_FRACTION;

  // Ctrl combos match the physical key: macOS WKWebView turns e.key into a control char when Ctrl is held (see KeyboardShortcuts / LayerStack).
  const ctrlOnly = e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey;
  if (ctrlOnly && e.code === 'KeyD') return viewportHeight / 2;
  if (ctrlOnly && e.code === 'KeyU') return -viewportHeight / 2;

  return null;
}

export function createVimScroll(): VimScroll {
  // True after a lone `g`, while we wait to see whether the next key completes `gg`.
  let pendingG = false;

  function handleKey(e: KeyboardEvent, container: HTMLElement): boolean {
    const completingG = pendingG;
    pendingG = false;

    const noMods = !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey;

    // The edge jumps write scrollTop directly: an instant scrollTo does not interrupt an in-flight smooth scroll in WebKit, but assigning scrollTop does, so gg/G land reliably even mid-animation.
    if (noMods && e.key === 'g') {
      if (completingG) {
        container.scrollTop = 0;
        return true;
      }
      pendingG = true;
      return true;
    }

    if (!e.ctrlKey && !e.altKey && !e.metaKey && e.key === 'G') {
      container.scrollTop = container.scrollHeight;
      return true;
    }

    const delta = pagerScrollDelta(e, container.clientHeight);
    if (delta === null) return false;
    // Animate the relative nudges so held j/k reads as continuous motion rather than discrete jumps; the gg/G edge jumps stay instant.
    container.scrollBy({ top: delta, behavior: 'smooth' });
    return true;
  }

  return {
    handleKey,
    reset() {
      pendingG = false;
    },
  };
}
