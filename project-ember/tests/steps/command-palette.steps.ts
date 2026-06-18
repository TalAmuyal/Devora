import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { ensureWsHubClosed } from '../support/ws-hub-helper';
import {
  dispatchShiftShift,
  openCommandPalette,
  getCommandItemCount,
  getSelectedCommandTitle,
  isSearchFieldFocused,
  pressEscapeInSearchField,
} from '../support/command-palette-helper';

Given('no overlay is open', async function (this: EmberWorld) {
  await ensureWsHubClosed(this.driver);
  await this.driver.pollFor(
    'return window.__test.overlayManager.isTabCoveringOverlayActive()',
    false,
    3_000,
  );
});

Given('the Command Palette is open', async function (this: EmberWorld) {
  await ensureWsHubClosed(this.driver);
  await openCommandPalette(this.driver);
});

When('the user opens the Command Palette via shift+shift', async function (this: EmberWorld) {
  await dispatchShiftShift(this.driver);
  // Give the toggle a moment to (not) open the overlay before asserting.
  await new Promise((r) => setTimeout(r, 200));
});

When(
  'the user presses {string} without first taking focus',
  async function (this: EmberWorld, key: string) {
    const ui = new UIDriver(this.driver);
    await ui.pressKeyRaw(key);
    await new Promise((r) => setTimeout(r, 150));
  },
);

When('the user presses Escape in the search field', async function (this: EmberWorld) {
  await pressEscapeInSearchField(this.driver);
  await new Promise((r) => setTimeout(r, 150));
});

When('the user presses Ctrl+S', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.pressKey('s', { ctrlKey: true, code: 'KeyS' });
  await new Promise((r) => setTimeout(r, 200));
});

When('the user filters commands by {string}', async function (this: EmberWorld, text: string) {
  // The search field is focused on open, so the user just types — no need to focus it first.
  const ui = new UIDriver(this.driver);
  await ui.typeIntoInput('.search-input-field', text);
  await new Promise((r) => setTimeout(r, 200));
});

Then('the Command Palette should be visible', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return document.querySelector('.command-palette') !== null`,
    true,
    3_000,
  );
});

Then('the Command Palette should not be visible', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return document.querySelector('.command-palette') !== null`,
    false,
    3_000,
  );
});

Then('the search field should be focused', async function (this: EmberWorld) {
  // Poll to tolerate WKWebView landing input focus a tick after open.
  await this.driver.pollFor(
    `return document.activeElement?.classList.contains('search-input-field') ?? false`,
    true,
    3_000,
  );
  const focused = await isSearchFieldFocused(this.driver);
  assert.strictEqual(focused, true, 'the search field should be focused');
});

Then(
  'the Command Palette should show {int} command(s)',
  async function (this: EmberWorld, expected: number) {
    await this.driver.pollFor(
      `return document.querySelectorAll('.command-palette-item').length`,
      expected,
      3_000,
    );
    const count = await getCommandItemCount(this.driver);
    assert.strictEqual(count, expected);
  },
);

Then(
  'the Command Palette should show at least {int} command(s)',
  async function (this: EmberWorld, minimum: number) {
    await this.driver.pollFor(
      `return document.querySelectorAll('.command-palette-item').length >= ${minimum}`,
      true,
      3_000,
    );
    const count = await getCommandItemCount(this.driver);
    assert.ok(count >= minimum, `expected at least ${minimum} commands, got ${count}`);
  },
);

Then(
  'the selected command should be {string}',
  async function (this: EmberWorld, expected: string) {
    const title = await getSelectedCommandTitle(this.driver);
    assert.strictEqual(title, expected);
  },
);

Then('the Workspace Hub overlay should be present', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return document.querySelector('.ws-hub') !== null`,
    true,
    3_000,
  );
});

Then('the Workspace Hub overlay should not be present', async function (this: EmberWorld) {
  const present = await this.driver.eval(
    `return document.querySelector('.ws-hub') !== null`,
  );
  assert.strictEqual(present, false, 'Workspace Hub should not be present');
});
