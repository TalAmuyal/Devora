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
 * This goes through the real open path, which focuses the search field — so the user can type a filter immediately (mirroring real usage).
 */
export async function openCommandPalette(driver: AppDriver): Promise<void> {
  await driver.eval(`window.__test.openCommandPalette()`);
  await driver.pollFor(
    `return document.querySelector('.command-palette') !== null`,
    true,
    5_000,
  );
}

/** Whether the palette's search field currently holds keyboard focus. */
export async function isSearchFieldFocused(driver: AppDriver): Promise<boolean> {
  return await driver.eval(
    `return document.activeElement?.classList.contains('search-input-field') ?? false`,
  );
}

/**
 * Dispatch Escape on the focused search field, as a real keypress would.
 * pressKey blurs the field first and pressKeyRaw dispatches on window, so neither reaches the field's own handler that owns the type-first close path.
 */
export async function pressEscapeInSearchField(driver: AppDriver): Promise<void> {
  await driver.eval(`
    const input = document.querySelector('.search-input-field');
    if (!input) throw new Error('Command Palette search field not found');
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }));
  `);
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
