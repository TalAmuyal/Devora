import { AppDriver } from './app-driver';

/**
 * Evaluate JavaScript inside an overlay iframe via the postMessage bridge.
 *
 * The bridge script (injected by Tauri into all frames at document start)
 * listens for `devora-eval-bridge` messages and relays results back to the
 * parent frame as `devora-eval-result` messages.
 */
export async function evalInOverlay(
  driver: AppDriver,
  js: string,
  iframeSelector = '.web-content-iframe',
  timeoutMs = 10_000,
): Promise<any> {
  const id = crypto.randomUUID();

  return driver.eval(`
    const iframe = document.querySelector(${JSON.stringify(iframeSelector)});
    if (!iframe) throw new Error('Overlay iframe not found: ' + ${JSON.stringify(iframeSelector)});
    if (!iframe.contentWindow) throw new Error('Overlay iframe has no contentWindow');

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        window.removeEventListener('message', handler);
        reject(new Error('evalInOverlay timed out after ${timeoutMs}ms'));
      }, ${timeoutMs});

      function handler(event) {
        if (event.data && event.data.type === 'devora-eval-result' && event.data.id === ${JSON.stringify(id)}) {
          clearTimeout(timeout);
          window.removeEventListener('message', handler);
          if (event.data.error) {
            reject(new Error(event.data.error));
          } else {
            resolve(event.data.result);
          }
        }
      }

      window.addEventListener('message', handler);
      iframe.contentWindow.postMessage({
        type: 'devora-eval-bridge',
        id: ${JSON.stringify(id)},
        js: ${JSON.stringify(js)}
      }, '*');
    });
  `);
}

/**
 * Repeatedly evaluate JavaScript inside an overlay iframe until the result
 * matches the expected value (compared via JSON serialization).
 */
export async function pollInOverlay(
  driver: AppDriver,
  js: string,
  expected: any,
  iframeSelector = '.web-content-iframe',
  timeoutMs = 10_000,
): Promise<any> {
  const expectedJson = JSON.stringify(expected);
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const result = await evalInOverlay(driver, js, iframeSelector, 5_000);
      if (JSON.stringify(result) === expectedJson) return result;
    } catch {
      // The iframe bridge may not be ready yet (page still loading).
      // Swallow and retry.
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(
    `pollInOverlay timed out after ${timeoutMs}ms: expected ${expectedJson}`,
  );
}
