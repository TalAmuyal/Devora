import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as path from 'node:path';
import {
  createTestFixtureRoot,
  createTestProfile,
  writeTestConfig,
} from '../../../project-ember/tests/support/fixture-helper';
import type { ProfileFormState } from '../support/bridge';
import { SapphireWorld } from '../support/world';
import { clickMenuItem } from './workspace-hub.steps';

// The phrases are Ember's (`project-ember/tests/steps/profile-management.steps.ts`); the form's fields are filled in through the bridge, as Ember's steps set the inputs' values, and its typing is the XCUITests'.

Given('no profiles are configured', function (this: SapphireWorld) {
  this.fixtureRoot ??= createTestFixtureRoot();
  this.profilePaths = [];
  writeTestConfig(this.testConfigPath, []);
});

Given('an unregistered initialized profile directory {string}', function (this: SapphireWorld, name: string) {
  this.fixtureRoot ??= createTestFixtureRoot();
  createTestProfile(this.fixtureRoot, name);
});

Then('the first-run welcome card should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the first-run welcome', () => this.bridge.state(), (s) => s.hub?.welcome === true);
});

Then('pressing {string} should not close the Workspace Hub', async function (this: SapphireWorld, key: string) {
  await this.bridge.press(key);
  // Give a dismissal time to happen if the lock failed.
  await new Promise((resolve) => setTimeout(resolve, 300));
  const { hub, windowLayers } = await this.bridge.state();
  assert.ok(windowLayers.includes('workspace-hub') && hub?.welcome, `the hub should stay open on its welcome after "${key}"`);
});

When('the user clicks the {string} dropdown action', async function (this: SapphireWorld, label: string) {
  await clickMenuItem(this, 'profiles', label);
});

When(
  'the user enters profile name {string} and path {string} under the fixture root',
  async function (this: SapphireWorld, name: string, relativePath: string) {
    await fillProfileForm(this, { name, path: path.join(this.fixtureRoot!, relativePath) });
  },
);

When(
  'the user enters path {string} under the fixture root in the profile form',
  async function (this: SapphireWorld, relativePath: string) {
    await fillProfileForm(this, { path: path.join(this.fixtureRoot!, relativePath) });
  },
);

When('the user submits the profile form', async function (this: SapphireWorld) {
  await waitForForm(this, 'the submit button to be enabled', (form) => form.canSubmit);
  await this.bridge.profileForm({ action: 'submit' });
  // Registering closes the form, and the hub shows the new profile's workspaces.
  await this.bridge.waitFor(
    'the profile form to close over the loaded hub',
    () => this.bridge.state(),
    (s) => s.profileForm === null && s.hub !== null && !s.hub.loading,
  );
});

Then('the profile form should detect the existing profile {string}', async function (this: SapphireWorld, name: string) {
  await waitForForm(
    this,
    `the existing profile "${name}"`,
    (form) => form.status?.kind === 'existing' && form.status.name === name && form.nameLocked && form.name === name,
  );
});

Then('the profile form submit button should read {string}', async function (this: SapphireWorld, label: string) {
  assert.strictEqual((await this.bridge.state()).profileForm?.submitLabel, label);
});

Then('the global config should list {int} profile(s)', function (this: SapphireWorld, expected: number) {
  const config = JSON.parse(fs.readFileSync(this.testConfigPath, 'utf-8'));
  assert.strictEqual((config.profiles ?? []).length, expected, `profiles: ${JSON.stringify(config.profiles)}`);
});

Then('the directory {string} should exist under the fixture root', function (this: SapphireWorld, relativePath: string) {
  const dir = path.join(this.fixtureRoot!, relativePath);
  assert.ok(fs.existsSync(dir), `${dir} should exist`);
});

/** Fills in the name, if given, then the path, and waits for the path's validation, as Ember's `fillProfileForm` does. */
async function fillProfileForm(world: SapphireWorld, fields: { name?: string; path: string }): Promise<void> {
  await waitForForm(world, 'the profile form', () => true);
  if (fields.name !== undefined) await world.bridge.profileForm({ action: 'setName', value: fields.name });
  await world.bridge.profileForm({ action: 'setPath', value: fields.path });
  await waitForForm(world, `the validation of ${fields.path}`, (form) => form.path === fields.path && !form.validating && form.status !== null);
}

async function waitForForm(world: SapphireWorld, what: string, done: (form: ProfileFormState) => boolean): Promise<ProfileFormState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.profileForm !== null && done(s.profileForm));
  return state.profileForm!;
}
