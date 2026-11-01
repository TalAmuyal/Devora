import assert from 'node:assert';
import { When, Then } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as path from 'node:path';
import type { SettingState, SettingsState } from '../support/bridge';
import { SapphireWorld } from '../support/world';
import { clickMenuItem } from './workspace-hub.steps';

// The phrases are Ember's (`project-ember/tests/steps/profile-management.steps.ts`, `claude-config.steps.ts`), with Sapphire's own for the settings cards; a click is the bridge's `settings` or `dialog` route, which runs what the control runs.

async function waitForSettings(world: SapphireWorld, what: string, done: (settings: SettingsState) => boolean): Promise<SettingsState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.settings !== null && done(s.settings));
  return state.settings!;
}

async function waitForSetting(world: SapphireWorld, key: string, what: string, done: (setting: SettingState | undefined) => boolean): Promise<void> {
  await waitForSettings(world, `${key} ${what}`, (settings) => {
    if (!settings.scope?.loaded) return false;
    return done(settings.scope.fields!.find((field) => field.key === key));
  });
}

Then('the Settings Hub should be visible', async function (this: SapphireWorld) {
  await waitForSettings(this, 'the Settings Hub', (settings) => settings.loaded);
});

Then('the Settings Hub should list {int} profile(s) and a New Profile row', async function (this: SapphireWorld, count: number) {
  const settings = await waitForSettings(
    this,
    `${count} profiles in the Settings Hub`,
    (settings) => settings.loaded && settings.rows.filter((row) => row.kind === 'profile').length === count,
  );
  assert.strictEqual(settings.rows.filter((row) => row.kind === 'newProfile').length, 1, 'the pinned New Profile row');
});

Then('the focused profile should be {string}', async function (this: SapphireWorld, name: string) {
  await waitForSettings(this, `${name} to be focused`, (settings) => settings.focused === name);
});

When('the user opens the {string} settings detail', async function (this: SapphireWorld, label: string) {
  await waitForSettings(this, 'the Settings Hub', (settings) => settings.loaded);
  await this.bridge.settings({ action: 'selectRow', label });
  await waitForSettings(this, `the ${label} settings`, (settings) => settings.focused === label && settings.scope?.loaded === true);
});

Then('the profile directory {string} should still exist on disk', function (this: SapphireWorld, name: string) {
  const config = path.join(this.fixtureRoot!, name, 'config.json');
  assert.ok(fs.existsSync(config), `${config} should remain on disk`);
});

Then('the confirmation dialog should mention {string}', async function (this: SapphireWorld, text: string) {
  const { dialog } = await this.bridge.waitFor('a confirmation dialog', () => this.bridge.state(), (s) => s.dialog !== null);
  const content = [dialog!.title, dialog!.intro, ...dialog!.items.map((item) => `${item.name} ${item.detail}`)].join('\n');
  assert.ok(content.includes(text), `the dialog should mention "${text}", got: ${content}`);
});

Then('exactly {int} confirmation dialog(s) should be visible', async function (this: SapphireWorld, count: number) {
  // Long enough for a second dialog that was going to open.
  await new Promise((r) => setTimeout(r, 300));
  const { windowLayers } = await this.bridge.state();
  assert.strictEqual(windowLayers.filter((name) => name === 'confirmation-dialog').length, count, JSON.stringify(windowLayers));
});

Then('a notice dialog titled {string} should be visible', async function (this: SapphireWorld, title: string) {
  await this.bridge.waitFor(`a notice titled "${title}"`, () => this.bridge.state(), (s) => s.dialog?.title.includes(title) ?? false);
});

Then('the notice dialog should have no cancel button', async function (this: SapphireWorld) {
  assert.strictEqual((await this.bridge.state()).dialog?.cancelLabel, null);
});

Then('a workspace session should be open', async function (this: SapphireWorld) {
  await this.bridge.waitFor('a workspace session', () => this.bridge.state(), (s) => s.tabs.some((tab) => tab.kind === 'workspace'));
});

When('the user opens Settings from the Workspace Hub menu', async function (this: SapphireWorld) {
  await clickMenuItem(this, 'main', 'Settings');
});

Then('the profile dropdown should list profile {string}', async function (this: SapphireWorld, name: string) {
  const items = (await this.bridge.state()).hub?.menu?.items ?? [];
  assert.ok(items.some((item) => item.kind === 'option' && item.label === name), JSON.stringify(items));
});

Then('the profile dropdown should offer {string} and {string}', async function (this: SapphireWorld, first: string, second: string) {
  const actions = ((await this.bridge.state()).hub?.menu?.items ?? []).filter((item) => item.kind === 'action');
  for (const label of [first, second]) {
    assert.ok(actions.some((item) => item.label === label && item.enabled), `"${label}" should be offered: ${JSON.stringify(actions)}`);
  }
});

// The settings cards.

When('the user sets {string} to {string}', async function (this: SapphireWorld, key: string, value: string) {
  await this.bridge.settings({ action: 'selectSegment', key, label: 'Set' });
  await waitForSetting(this, key, 'to show its input', (setting) => setting?.input != null);
  await this.bridge.settings({ action: 'type', key, value });
  await this.bridge.settings({ action: 'commit', key });
});

When('the user switches {string} to {string}', async function (this: SapphireWorld, key: string, label: string) {
  await this.bridge.settings({ action: 'selectSegment', key, label });
});

Then('the setting {string} should show the hint {string}', async function (this: SapphireWorld, key: string, hint: string) {
  await waitForSetting(this, key, `to show the hint "${hint}"`, (setting) => setting?.hint === hint && setting.input == null);
});

Then('the setting {string} should be shown', async function (this: SapphireWorld, key: string) {
  await waitForSetting(this, key, 'to be shown', (setting) => setting !== undefined);
});

Then('the setting {string} should not be shown', async function (this: SapphireWorld, key: string) {
  await waitForSetting(this, key, 'to be hidden', (setting) => setting === undefined);
});

/** The value at a dot-path key of a config file, polled since the write lands after the bridge call returns. */
async function expectConfig(file: string, key: string, expected: unknown): Promise<void> {
  const deadline = Date.now() + 5_000;
  let actual: unknown;
  for (;;) {
    const config = JSON.parse(fs.readFileSync(file, 'utf-8'));
    actual = key.split('.').reduce<any>((node, part) => (node == null ? undefined : node[part]), config);
    if (actual === expected) return;
    if (Date.now() > deadline) assert.fail(`${key} in ${file} should be ${JSON.stringify(expected)}, is ${JSON.stringify(actual)}`);
    await new Promise((r) => setTimeout(r, 100));
  }
}

function profileConfig(world: SapphireWorld, name: string): string {
  return path.join(world.fixtureRoot!, name, 'config.json');
}

Then('the global config should have {string} set to {string}', async function (this: SapphireWorld, key: string, value: string) {
  await expectConfig(this.testConfigPath, key, value);
});

Then('the profile {string} config should have {string} set to {string}', async function (this: SapphireWorld, name: string, key: string, value: string) {
  await expectConfig(profileConfig(this, name), key, value);
});

Then('the profile {string} config should have {string} set to false', async function (this: SapphireWorld, name: string, key: string) {
  await expectConfig(profileConfig(this, name), key, false);
});

Then('the profile {string} config should not have {string}', async function (this: SapphireWorld, name: string, key: string) {
  await expectConfig(profileConfig(this, name), key, undefined);
});
