import { initWindowLayerStack } from '../../layers/stack';
import { LayerRouter } from '../../layers/LayerRouter';
import { LayerStack } from '../../layers/LayerStack';

/**
 * Install a real window stack + router for modal-dialog unit tests: the dialogs mount themselves through `getWindowLayerStack()`, so their keys route through the same dispatcher production uses.
 * Pair `installModalStack()` in `beforeEach` with `teardownModalStack()` in `afterEach`.
 */

let stack: LayerStack | null = null;
let router: LayerRouter | null = null;

export function installModalStack(): LayerStack {
  const host = document.createElement('div');
  document.body.appendChild(host);
  stack = initWindowLayerStack({ host });
  router = new LayerRouter({ windowStack: stack, resolveTabStack: () => null });
  router.install();
  return stack;
}

export function teardownModalStack(): void {
  router?.uninstall();
  router = null;
  stack?.clear();
  stack = null;
  document.body.innerHTML = '';
}

/** Dispatch a keydown on `window`, exactly where the router listens. */
export function pressKey(init: KeyboardEventInit): KeyboardEvent {
  const e = new KeyboardEvent('keydown', { bubbles: true, cancelable: true, ...init });
  window.dispatchEvent(e);
  return e;
}
