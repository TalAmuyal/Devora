import { AppDriver } from './app-driver';
import { UIDriver } from './ui-driver';

export async function waitForProfileManager(driver: AppDriver): Promise<void> {
  await driver.pollFor(
    `return document.querySelector('.profile-manager') !== null`,
    true,
    5_000,
  );
}

/** Wait until the master list shows the expected number of profile rows (excluding the pinned "New Profile…" row). */
export async function waitForProfileItems(
  driver: AppDriver,
  expectedCount: number,
  timeoutMs = 5_000,
): Promise<void> {
  await driver.pollFor(
    `return document.querySelectorAll('.pm-master-item:not(.pm-new-row)').length`,
    expectedCount,
    timeoutMs,
  );
}

export async function getFocusedProfileName(driver: AppDriver): Promise<string | null> {
  return await driver.eval(
    `return document.querySelector('.pm-master-focused .pm-name')?.textContent?.trim() ?? null`,
  );
}

/** Fill the profile form (Profile Manager detail panel or first-run welcome).
 * Types the path directly — never clicks Browse…, which would open a native macOS dialog that the harness cannot drive.
 * Waits for the path validation round-trip to settle (the status line appears).
 */
export async function fillProfileForm(
  driver: AppDriver,
  fields: { name?: string; path: string },
): Promise<void> {
  const ui = new UIDriver(driver);
  if (fields.name !== undefined) {
    await ui.typeIntoInput('.pm-form-name', fields.name);
  }
  await ui.typeIntoInput('.pm-form-path', fields.path);
  await driver.pollFor(
    `const status = document.querySelector('.pm-form-status');
     return status !== null && status.style.display !== 'none' && status.textContent !== ''`,
    true,
    5_000,
  );
}

/** Submit the profile form and block until registration completed: the form is gone and the Workspace Hub is showing again. */
export async function submitProfileForm(driver: AppDriver): Promise<void> {
  await driver.pollFor(
    `return document.querySelector('.pm-form-submit')?.disabled === false`,
    true,
    5_000,
  );
  const ui = new UIDriver(driver);
  await ui.click('.pm-form-submit');
  await driver.pollFor(
    `return document.querySelector('.pm-form') === null
         && document.querySelector('.ws-hub') !== null`,
    true,
    10_000,
  );
}
