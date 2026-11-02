import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';

function setClaude(world: SapphireWorld, key: string, value: string | null): void {
  const config = JSON.parse(fs.readFileSync(world.testConfigPath, 'utf8'));
  config.claude ??= {};
  config.claude[key] = value;
  fs.writeFileSync(world.testConfigPath, JSON.stringify(config));
}

Given('the global config sets the Claude {string} to {string}', function (this: SapphireWorld, key: string, value: string) {
  setClaude(this, key, value);
});
Given('the global config sets the Claude {string} to None', function (this: SapphireWorld, key: string) {
  setClaude(this, key, null);
});

export async function waitForClaude(world: SapphireWorld): Promise<void> {
  await world.bridge.waitFor('Claude settings to load', () => world.bridge.state(), (s) => s.settings?.scope?.claude?.loaded === true);
}

When('the user sets the effort level to {string}', async function (this: SapphireWorld, level: string) {
  await waitForClaude(this);
  await this.bridge.settings({ action: 'selectSegment', key: 'effort', label: level });
});
When('the user turns the default model off', async function (this: SapphireWorld) {
  await waitForClaude(this);
  await this.bridge.settings({ action: 'selectSegment', key: 'default-model', label: 'Off' });
});
When('the user sets the permission mode to {string}', async function (this: SapphireWorld, mode: string) {
  await waitForClaude(this);
  await this.bridge.settings({ action: 'selectSegment', key: 'permission-mode', label: 'Custom' });
  await this.bridge.settings({ action: 'type', key: 'permission-mode', value: mode });
  await this.bridge.settings({ action: 'commit', key: 'permission-mode' });
});

Then('the global config should have the Claude {string} set to {string}', async function (this: SapphireWorld, key: string, value: string) {
  const config = await this.bridge.waitFor(`claude.${key} to be written`, async () => JSON.parse(fs.readFileSync(this.testConfigPath, 'utf8')), (c) => c.claude?.[key] === value);
  assert.equal(config.claude[key], value);
});
Then('the global config should have the Claude {string} set to None', async function (this: SapphireWorld, key: string) {
  const config = await this.bridge.waitFor(`claude.${key} to be None`, async () => JSON.parse(fs.readFileSync(this.testConfigPath, 'utf8')), (c) => c.claude?.[key] === null);
  assert.equal(config.claude[key], null);
});

When('the user switches Claude {string} to {string}', async function (this: SapphireWorld, key: string, label: string) {
  await waitForClaude(this);
  await this.bridge.settings({ action: 'selectSegment', key, label });
});
When('the user selects the Claude {string} suggestion {string}', async function (this: SapphireWorld, key: string, label: string) {
  await this.bridge.waitFor(`Claude ${key} suggestion ${label}`, () => this.bridge.state(), (s) =>
    s.settings?.scope?.claude?.fields.some((f) => f.key === key && f.suggestions.includes(label)) === true);
  await this.bridge.settings({ action: 'suggest', key, label });
});
When('the user types Claude {string} draft {string}', async function (this: SapphireWorld, key: string, value: string) {
  await this.bridge.settings({ action: 'type', key, value });
});
When('the user sets Claude {string} to custom {string}', async function (this: SapphireWorld, key: string, value: string) {
  await waitForClaude(this);
  await this.bridge.settings({ action: 'selectSegment', key, label: 'Custom' });
  await this.bridge.settings({ action: 'type', key, value });
  await this.bridge.settings({ action: 'commit', key });
});

Then('Claude {string} should show {string} with hint {string}', async function (this: SapphireWorld, key: string, segment: string, hint: string) {
  await this.bridge.waitFor(`Claude ${key} to show ${segment} / ${hint}`, () => this.bridge.state(), (s) =>
    s.settings?.scope?.claude?.fields.some((f) => f.key === key && f.segment === segment && f.hint === hint) === true);
});
Then('Claude {string} should have draft {string}', async function (this: SapphireWorld, key: string, value: string) {
  await this.bridge.waitFor(`Claude ${key} draft`, () => this.bridge.state(), (s) =>
    s.settings?.scope?.claude?.fields.some((f) => f.key === key && f.segment === 'Custom' && f.input === value && f.showsSave) === true);
});
Then('the user config should not store Claude {string}', function (this: SapphireWorld, key: string) {
  const config = JSON.parse(fs.readFileSync(this.testConfigPath, 'utf8'));
  assert.equal(Object.hasOwn(config.claude ?? {}, key), false);
});
Then('the profile {string} Claude {string} should be None on disk', async function (this: SapphireWorld, profile: string, key: string) {
  await this.bridge.waitFor(`profile ${key} to be null`, async () => JSON.parse(fs.readFileSync(path.join(this.fixtureRoot!, profile, 'config.json'), 'utf8')), (c) => c.claude?.[key] === null);
});

const FIXTURE_TOKEN = 'fixture-token-only';
When('the user saves the fixture Asana token', async function (this: SapphireWorld) {
  await this.bridge.settings({ action: 'type', key: 'asana-token', value: FIXTURE_TOKEN });
  await this.bridge.settings({ action: 'save', key: 'asana-token' });
});
When('the user clears the Asana token', async function (this: SapphireWorld) {
  await this.bridge.settings({ action: 'clear', key: 'asana-token' });
});
Then('the Asana token should be {string}', async function (this: SapphireWorld, label: string) {
  assert.ok(['Set', 'Not set'].includes(label));
  await this.bridge.waitFor(`Asana token ${label}`, () => this.bridge.state(), (s) => s.settings?.scope?.asana?.isSet === (label === 'Set'));
});
Then('the Asana token draft should be empty', async function (this: SapphireWorld) {
  await this.bridge.waitFor('empty token draft', () => this.bridge.state(), (s) => s.settings?.scope?.asana?.hasDraft === false);
});
Then('the Asana token row should be hidden', async function (this: SapphireWorld) {
  await this.bridge.waitFor('no token row', () => this.bridge.state(), (s) => s.settings?.scope?.loaded === true && s.settings.scope.asana === undefined);
});
Then('neither scope should store the fixture token on disk', function (this: SapphireWorld) {
  for (const file of [this.testConfigPath, path.join(this.fixtureRoot!, 'Work', 'config.json')]) {
    assert.equal(fs.readFileSync(file, 'utf8').includes(FIXTURE_TOKEN), false, 'tokens belong to the credential store');
  }
});
