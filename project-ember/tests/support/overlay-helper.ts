import assert from 'node:assert';
import { AppDriver } from './app-driver';

export async function assertOverlayHeader(
  driver: AppDriver,
  expectedText: string,
  timeoutMs = 30_000,
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const text: string | null = await driver.eval(
      `return document.querySelector('.web-content-header span')?.textContent ?? null`,
    );
    if (text !== null && text.includes(expectedText)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(
    `Overlay header did not contain "${expectedText}" within ${timeoutMs}ms`,
  );
}

export async function assertOverlayIframeSrc(
  driver: AppDriver,
  pattern: RegExp,
  timeoutMs = 30_000,
): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const src: string | null = await driver.eval(
      `return document.querySelector('.web-content-iframe')?.src ?? null`,
    );
    if (src !== null && pattern.test(src)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(
    `Overlay iframe src did not match ${pattern} within ${timeoutMs}ms`,
  );
}

export async function assertActivePanelOverlay(
  driver: AppDriver,
  expected: boolean,
  timeoutMs = 5_000,
): Promise<void> {
  const has = await driver.pollFor(
    `return window.__test.overlayManager.hasPanelOverlay(
       window.__test.sessionManager.getActiveSessionId()
     )`,
    expected,
    timeoutMs,
  );
  assert.strictEqual(has, expected);
}

export async function assertSessionPanelOverlay(
  driver: AppDriver,
  sessionIndex: number,
  expected: boolean,
  timeoutMs = 5_000,
): Promise<void> {
  const has = await driver.pollFor(
    `return window.__test.overlayManager.hasPanelOverlay(
       window.__test.sessionManager.getSessions()[${sessionIndex}]?.id
     )`,
    expected,
    timeoutMs,
  );
  assert.strictEqual(has, expected);
}

export async function assertPanelOverlayVisible(
  driver: AppDriver,
  expected: boolean,
): Promise<void> {
  const visible = await driver.eval(
    'return window.__test.overlayManager.hasAnyVisiblePanelOverlay()',
  );
  assert.strictEqual(
    visible,
    expected,
    `Expected panel overlay to be ${expected ? 'visible' : 'hidden'}, but it was ${visible ? 'visible' : 'hidden'}`,
  );
}
