import { initLayerStack } from '../../layers/stack';
import { LayerStack } from '../../layers/LayerStack';

/**
 * Install a real LayerStack singleton for modal-dialog unit tests: the dialogs mount themselves as `modal` layers through `getLayerStack()`, so their keys route through the same window-capture dispatcher production uses.
 * Pair `installModalStack()` in `beforeEach` with `teardownModalStack()` in `afterEach`.
 */

let stack: LayerStack | null = null;

export function installModalStack(): LayerStack {
  const pageHost = document.createElement('div');
  document.body.appendChild(pageHost);
  stack = initLayerStack({
    pageHost,
    modalHost: document.body,
    resolveBaseFocus: () => null,
  });
  stack.install();
  return stack;
}

export function teardownModalStack(): void {
  if (stack) {
    stack.clear();
    stack.uninstall();
    stack = null;
  }
  document.body.innerHTML = '';
}

/** Dispatch a keydown on `window`, exactly where the LayerStack listens. */
export function pressKey(init: KeyboardEventInit): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(e);
  return e;
}
