import { AppDriver } from './app-driver';
import { UIDriver } from './ui-driver';

export async function ensureWsHubOpen(driver: AppDriver): Promise<void> {
  await driver.eval(`
    if (!window.__test.overlayManager.isTabCoveringOverlayActive()) {
      await window.__test.wsHub.load();
      window.__test.overlayManager.showTabCoveringOverlay(window.__test.wsHub.getElement());
    }
  `);
  await driver.pollFor(
    `return document.querySelector('.ws-hub') !== null`,
    true,
    5_000,
  );
}

export async function ensureWsHubClosed(driver: AppDriver): Promise<void> {
  await driver.eval(`
    if (window.__test.overlayManager.isTabCoveringOverlayActive()) {
      window.__test.wsHub.unload();
      window.__test.overlayManager.dismissTabCoveringOverlay();
    }
  `);
}

export async function reloadWsHub(driver: AppDriver): Promise<void> {
  await driver.eval(`
    window.__test.wsHub.unload();
    window.__test.overlayManager.dismissTabCoveringOverlay();
    window.__test.wsHub.activeProfilePath = null;
    await window.__test.wsHub.load();
    window.__test.overlayManager.showTabCoveringOverlay(window.__test.wsHub.getElement());
  `);
  await driver.pollFor(
    `return document.querySelector('.ws-master-item') !== null
         || document.querySelector('.ws-empty-message') !== null`,
    true,
    5_000,
  );
  // Blur any element that may have received focus during render (e.g. the
  // search input in WebKit) so the panel starts in normal navigation mode.
  await driver.eval(`document.activeElement?.blur()`);
}

export async function getFocusedWorkspaceId(driver: AppDriver): Promise<string | null> {
  return await driver.eval(
    `return document.querySelector('.ws-master-focused .ws-id')?.textContent?.trim() ?? null`,
  );
}

export async function getFocusedWorkspaceTitle(driver: AppDriver): Promise<string | null> {
  return await driver.eval(
    `return document.querySelector('.ws-master-focused .ws-task-name')?.textContent?.trim() ?? null`,
  );
}

export async function getWorkspaceItemCount(driver: AppDriver): Promise<number> {
  return await driver.eval(
    `return document.querySelectorAll('.ws-master-item').length`,
  );
}

export async function getActiveCategoryFilter(driver: AppDriver): Promise<string> {
  return await driver.eval(`
    const btn = document.querySelector('.ws-category-active');
    return btn?.textContent?.replace(/\\d/g, '')?.trim() ?? '';
  `);
}

export async function waitForWorkspaceItems(
  driver: AppDriver,
  expectedCount: number,
  timeoutMs = 5_000,
): Promise<void> {
  await driver.pollFor(
    `return document.querySelectorAll('.ws-master-item').length`,
    expectedCount,
    timeoutMs,
  );
}

export async function selectCategory(
  ui: UIDriver,
  category: 'active' | 'inactive' | 'all',
): Promise<void> {
  const keyMap: Record<typeof category, string> = {
    active: '1',
    inactive: '2',
    all: '3',
  };
  await ui.pressKey(keyMap[category]);
}

export async function filterWorkspaces(
  ui: UIDriver,
  driver: AppDriver,
  text: string,
): Promise<void> {
  await ui.pressKey('f');
  await ui.typeIntoInput('.ws-search input', text);
  await new Promise((r) => setTimeout(r, 200));
}
