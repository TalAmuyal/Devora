import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import {
  createTestFixtureRoot,
  createTestProfile,
  writeTestConfig,
} from '../support/fixture-helper';
import {
  waitForSettingsHub,
  waitForProfileItems,
  getFocusedProfileName,
  fillProfileForm,
  submitProfileForm,
} from '../support/settings-hub-helper';
import { clickBurgerMenuItem } from '../support/ws-hub-helper';

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

Given('no profiles are configured', function (this: EmberWorld) {
  this.fixtureRoot = createTestFixtureRoot();
  writeTestConfig(this.testConfigPath!, []);
});

Given(
  'an unregistered initialized profile directory {string}',
  function (this: EmberWorld, name: string) {
    if (!this.fixtureRoot) {
      this.fixtureRoot = createTestFixtureRoot();
    }
    // Full profile structure on disk, deliberately NOT added to the global config.
    createTestProfile(this.fixtureRoot, name);
  },
);

// ---------------------------------------------------------------------------
// First-run welcome
// ---------------------------------------------------------------------------

Then('the first-run welcome card should be visible', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.waitForElement('.ws-welcome', 5_000);
});

Then(
  'pressing {string} should not close the Workspace Hub',
  async function (this: EmberWorld, key: string) {
    const ui = new UIDriver(this.driver);
    await ui.pressKey(key);
    // Give a dismissal time to happen if the zero-profile lock failed.
    await new Promise((r) => setTimeout(r, 300));
    const stillOpen = await this.driver.eval(
      `return window.__test.overlayManager.isTabCoveringOverlayActive()
           && document.querySelector('.ws-welcome') !== null`,
    );
    assert.strictEqual(stillOpen, true, `Hub should stay open after pressing "${key}"`);
  },
);

// ---------------------------------------------------------------------------
// Profile form
// ---------------------------------------------------------------------------

When(
  'the user enters profile name {string} and path {string} under the fixture root',
  async function (this: EmberWorld, name: string, relativePath: string) {
    await fillProfileForm(this.driver, {
      name,
      path: path.join(this.fixtureRoot!, relativePath),
    });
  },
);

When(
  'the user enters path {string} under the fixture root in the profile form',
  async function (this: EmberWorld, relativePath: string) {
    await fillProfileForm(this.driver, {
      path: path.join(this.fixtureRoot!, relativePath),
    });
  },
);

When('the user submits the profile form', async function (this: EmberWorld) {
  await submitProfileForm(this.driver);
});

Then('the profile form should report a new profile', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return document.querySelector('.pm-form-status')?.classList.contains('ok') === true`,
    true,
    5_000,
  );
});

Then(
  'the profile form should detect the existing profile {string}',
  async function (this: EmberWorld, name: string) {
    await this.driver.pollFor(
      `const status = document.querySelector('.pm-form-status');
       return status?.classList.contains('detect') === true
           && status.textContent.includes(${JSON.stringify(name)})`,
      true,
      5_000,
    );
    const nameLocked = await this.driver.eval(
      `const input = document.querySelector('.pm-form-name');
       return input?.disabled === true && input.value === ${JSON.stringify(name)}`,
    );
    assert.strictEqual(nameLocked, true, `Name field should be locked to "${name}"`);
  },
);

Then(
  'the profile form submit button should read {string}',
  async function (this: EmberWorld, label: string) {
    const ui = new UIDriver(this.driver);
    const text = await ui.getTextContent('.pm-form-submit');
    assert.strictEqual(text, label);
  },
);

// ---------------------------------------------------------------------------
// Settings Hub page
// ---------------------------------------------------------------------------

Then('the Settings Hub should be visible', async function (this: EmberWorld) {
  await waitForSettingsHub(this.driver);
});

Then(
  'the Settings Hub should list {int} profile(s) and a New Profile row',
  async function (this: EmberWorld, expected: number) {
    await waitForProfileItems(this.driver, expected);
    const ui = new UIDriver(this.driver);
    const newRowCount = await ui.getElementCount('.pm-new-row');
    assert.strictEqual(newRowCount, 1, 'The pinned New Profile row should be present');
  },
);

Then(
  'the focused profile should be {string}',
  async function (this: EmberWorld, expected: string) {
    const name = await getFocusedProfileName(this.driver);
    assert.strictEqual(name, expected);
  },
);

Then(
  'the profile directory {string} should still exist on disk',
  function (this: EmberWorld, name: string) {
    const configPath = path.join(this.fixtureRoot!, name, 'config.json');
    assert.strictEqual(
      fs.existsSync(configPath),
      true,
      `Profile directory should remain on disk: ${configPath}`,
    );
  },
);

// ---------------------------------------------------------------------------
// Dialogs
// ---------------------------------------------------------------------------

Then(
  'the confirmation dialog should mention {string}',
  async function (this: EmberWorld, text: string) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.confirmation-dialog', 3_000);
    const content = await ui.getTextContent('.confirmation-dialog');
    assert.ok(content.includes(text), `Dialog should mention "${text}", got: ${content}`);
  },
);

Then(
  'a notice dialog titled {string} should be visible',
  async function (this: EmberWorld, title: string) {
    await this.driver.pollFor(
      `return (document.querySelector('.confirmation-dialog-title')?.textContent ?? '')
        .includes(${JSON.stringify(title)})`,
      true,
      5_000,
    );
  },
);

Then('the notice dialog should have no cancel button', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  const hasCancel = await ui.hasElement('.confirmation-dialog-cancel');
  assert.strictEqual(hasCancel, false, 'Notice dialog should not offer a cancel button');
});

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

Then('a workspace session should be open', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return window.__test.sessionManager.getSessions()
       .some((s) => s.profilePath !== null)`,
    true,
    10_000,
  );
});

// ---------------------------------------------------------------------------
// Hub profile dropdown
// ---------------------------------------------------------------------------

Then(
  'the profile dropdown should be labeled {string}',
  async function (this: EmberWorld, expected: string) {
    await this.driver.pollFor(
      `return document.querySelector('.ws-profile-dropdown .dropdown-trigger-label')
        ?.textContent ?? null`,
      expected,
      5_000,
    );
  },
);

When('the user clicks the profile dropdown', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.waitForElement('.ws-profile-dropdown .dropdown-trigger');
  await ui.click('.ws-profile-dropdown .dropdown-trigger');
  await ui.waitForElement('.ws-profile-dropdown .dropdown-popup', 3_000);
});

Then(
  'the profile dropdown should list profile {string}',
  async function (this: EmberWorld, name: string) {
    const found = await this.driver.eval(
      `return Array.from(
         document.querySelectorAll('.ws-profile-dropdown .dropdown-item-option .dropdown-item-label'),
       ).some((el) => el.textContent === ${JSON.stringify(name)})`,
    );
    assert.strictEqual(found, true, `Dropdown should list profile "${name}"`);
  },
);

Then(
  'the profile dropdown should offer {string} and {string}',
  async function (this: EmberWorld, first: string, second: string) {
    const labels: string[] = await this.driver.eval(
      `return Array.from(
         document.querySelectorAll('.ws-profile-dropdown .dropdown-item-action .dropdown-item-label'),
       ).map((el) => el.textContent)`,
    );
    assert.ok(labels.includes(first), `Dropdown should offer "${first}", got: ${labels}`);
    assert.ok(labels.includes(second), `Dropdown should offer "${second}", got: ${labels}`);
  },
);

When(
  'the user clicks the {string} dropdown action',
  async function (this: EmberWorld, label: string) {
    await this.driver.eval(
      `const rows = Array.from(
         document.querySelectorAll('.ws-profile-dropdown .dropdown-item-action'),
       );
       const row = rows.find(
         (r) => r.querySelector('.dropdown-item-label')?.textContent === ${JSON.stringify(label)}
       );
       if (!row) throw new Error('Dropdown action not found: ' + ${JSON.stringify(label)});
       row.click();`,
    );
  },
);

// ---------------------------------------------------------------------------
// Hub burger menu
// ---------------------------------------------------------------------------

When('the user opens Settings from the Workspace Hub menu', async function (this: EmberWorld) {
  // clickBurgerMenuItem throws if "Settings" is absent, so this also asserts the item is offered.
  await clickBurgerMenuItem(this.driver, 'Settings');
});

// ---------------------------------------------------------------------------
// Command palette
// ---------------------------------------------------------------------------

Then(
  'the palette should include the command {string}',
  async function (this: EmberWorld, title: string) {
    await this.driver.pollFor(
      `return Array.from(document.querySelectorAll('.command-palette-item-title'))
        .some((el) => el.textContent === ${JSON.stringify(title)})`,
      true,
      5_000,
    );
  },
);

// ---------------------------------------------------------------------------
// Global config assertions
// ---------------------------------------------------------------------------

Then(
  'the global config should list {int} profile(s)',
  function (this: EmberWorld, expected: number) {
    const config = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf-8'));
    assert.strictEqual(
      (config.profiles ?? []).length,
      expected,
      `Global config should list ${expected} profile(s), got: ${JSON.stringify(config.profiles)}`,
    );
  },
);

Then(
  'the directory {string} should exist under the fixture root',
  function (this: EmberWorld, relativePath: string) {
    const dir = path.join(this.fixtureRoot!, relativePath);
    assert.strictEqual(fs.existsSync(dir), true, `Directory should exist: ${dir}`);
  },
);
