import assert from 'node:assert';
import { AppDriver } from './app-driver';

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
