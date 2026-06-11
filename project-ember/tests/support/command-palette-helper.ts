import { AppDriver } from './app-driver';

/** Dispatch a rapid double-tap of Shift (the real Command Palette shortcut). */
export async function dispatchShiftShift(driver: AppDriver): Promise<void> {
  await driver.eval(`
    function shift(type) {
      window.dispatchEvent(new KeyboardEvent(type, {
        key: 'Shift',
        code: 'ShiftLeft',
        shiftKey: type === 'keydown',
        bubbles: true,
        cancelable: true,
      }));
    }
    shift('keydown');
    shift('keyup');
    shift('keydown');
    shift('keyup');
  `);
}

/**
 * Open the Command Palette deterministically via the wired open function.
 * This goes through the real open path, which focuses the palette — so keyboard
 * navigation works without any test-only blur (mirroring real usage).
 */
export async function openCommandPalette(driver: AppDriver): Promise<void> {
  await driver.eval(`window.__test.openCommandPalette()`);
  await driver.pollFor(
    `return document.querySelector('.command-palette') !== null`,
    true,
    5_000,
  );
}

export async function getCommandItemCount(driver: AppDriver): Promise<number> {
  return await driver.eval(
    `return document.querySelectorAll('.command-palette-item').length`,
  );
}

export async function getSelectedCommandTitle(driver: AppDriver): Promise<string | null> {
  return await driver.eval(
    `return document.querySelector('.command-palette-item.selected .command-palette-item-title')?.textContent?.trim() ?? null`,
  );
}
