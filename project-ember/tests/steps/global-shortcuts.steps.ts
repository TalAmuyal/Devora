import assert from 'node:assert';
import { When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';

// Both use pressKeyRaw: these shortcuts must work whatever holds focus, so blurring first would hide the very thing under test.
When('the user presses Ctrl+{int}', async function (this: EmberWorld, digit: number) {
  const ui = new UIDriver(this.driver);
  await ui.pressKeyRaw(String(digit), { ctrlKey: true, code: `Digit${digit}` });
  await new Promise((r) => setTimeout(r, 150));
});

// The chords of the shared `keyboard-shortcuts.feature`; `code` is the physical key, which the shortcuts match on.
const CHORDS: Record<string, { key: string; code: string; shiftKey?: boolean }> = {
  'Ctrl+Left': { key: 'ArrowLeft', code: 'ArrowLeft' },
  'Ctrl+Shift+Left': { key: 'ArrowLeft', code: 'ArrowLeft', shiftKey: true },
  'Ctrl+Shift+Right': { key: 'ArrowRight', code: 'ArrowRight', shiftKey: true },
  'Ctrl+Shift+S': { key: 'S', code: 'KeyS', shiftKey: true },
  'Ctrl+=': { key: '=', code: 'Equal' },
  'Ctrl+Shift+=': { key: '+', code: 'Equal', shiftKey: true },
  'Ctrl+Shift+-': { key: '_', code: 'Minus', shiftKey: true },
};
for (const [chord, { key, code, shiftKey }] of Object.entries(CHORDS)) {
  When(`the user presses ${chord}`, async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.pressKeyRaw(key, { ctrlKey: true, shiftKey, code });
    await new Promise((r) => setTimeout(r, 150));
  });
}

Then('the terminal font size should be {int}', async function (this: EmberWorld, size: number) {
  const actual = await this.driver.eval('return window.__test.sessionManager.getActiveSession()?.getFontSize()');
  assert.strictEqual(actual, size);
});

When('the user presses F1', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.pressKeyRaw('F1', { code: 'F1' });
  await new Promise((r) => setTimeout(r, 300));
});

Then('the UI font size should be {string}', async function (this: EmberWorld, size: string) {
  const actual = await this.driver.eval(
    'return document.documentElement.style.fontSize',
  );
  assert.strictEqual(actual, size);
});

Then('the User Guide should be visible', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return window.__test.layers.find('user-guide') !== null`,
    true,
    5_000,
  );
});

/**
 * Paint order is DOM order among a stack's wrappers, so "above" is "later in the host".
 * This is the assertion that replaced the hand-maintained z-index ladder: a page opened over a modal used to be painted underneath it.
 */
Then('the User Guide should be stacked above the dialog', async function (this: EmberWorld) {
  const guideIsLater = await this.driver.eval(`
    const wrappers = Array.from(document.getElementById('app').children)
      .filter((el) => el.classList.contains('layer-wrapper'));
    const guide = wrappers.findIndex((el) => el.querySelector('.web-content') !== null);
    const dialog = wrappers.findIndex((el) => el.classList.contains('layer-modal'));
    return guide !== -1 && dialog !== -1 && guide > dialog;
  `);
  assert.strictEqual(guideIsLater, true);
});
