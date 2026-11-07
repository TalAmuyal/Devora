import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';

When('the user saves Agent setting {string} with text:', async function (this: SapphireWorld, key: string, text: string) {
  await this.bridge.settings({ action: 'selectSegment', key, label: 'Set' });
  await this.bridge.waitFor('the Agent text editor', () => this.bridge.state(), state =>
    state.settings?.scope?.fields?.some(field => field.key === key && field.segment === 'Set') === true);
  await this.bridge.settings({ action: 'type', key, value: text });
  await this.bridge.settings({ action: 'save', key });
});

async function saved(file: string, description: string, select: (config: any) => unknown, expected: unknown): Promise<void> {
  const deadline = Date.now() + 5000;
  for (;;) {
    const value = select(JSON.parse(fs.readFileSync(file, 'utf8')));
    if (value === expected) return;
    if (Date.now() >= deadline) assert.fail(`${description}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(value)}`);
    await new Promise(resolve => setTimeout(resolve, 100));
  }
}

Then('the global Agent setting {string} should contain exactly:', async function (this: SapphireWorld, key: string, text: string) {
  await saved(this.testConfigPath, key, config => config.agent?.[key], text);
});

Then('the global config should store the Agent percentage {int}', async function (this: SapphireWorld, value: number) {
  await saved(this.testConfigPath, 'typed compact threshold', config => config.agent?.['compact-threshold'], value);
});

Then('the profile {string} config should store the Agent percentage {int}', async function (this: SapphireWorld, name: string, value: number) {
  await saved(path.join(this.fixtureRoot!, name, 'config.json'), 'typed profile compact threshold', config => config.agent?.['compact-threshold'], value);
});

async function expectPath(world: SapphireWorld, expected: string): Promise<void> {
  await world.bridge.waitFor('the resolved Agent template path', () => world.bridge.state(), state =>
    state.settings?.scope?.fields?.some(field => field.key === 'agent.template-file' && field.hint?.startsWith(expected + ' · ')) === true);
}

Then('the Agent template path should resolve relative to the global config as {string}', async function (this: SapphireWorld, relative: string) {
  await expectPath(this, path.join(path.dirname(this.testConfigPath), relative));
});

Then('the Agent template path should resolve relative to profile {string} as {string}', async function (this: SapphireWorld, profile: string, relative: string) {
  await expectPath(this, path.join(this.fixtureRoot!, profile, relative));
});

When('the user adds Agent input {string} with value {string}', async function (this: SapphireWorld, key: string, value: string) {
  await this.bridge.waitFor('Agent inputs to load', () => this.bridge.state(), state => state.settings?.scope?.agent?.loaded === true);
  await this.bridge.settings({ action: 'addAgentInput', key, value });
});

When('the user saves Agent input {string} as {string} with value {string}', async function (this: SapphireWorld, key: string, newKey: string, value: string) {
  await this.bridge.waitFor('the Agent input row', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.inputs.some(input => input.key === key) === true);
  await this.bridge.settings({ action: 'saveAgentInput', key, newKey, value });
});

When('the user defaults Agent input {string}', async function (this: SapphireWorld, key: string) {
  await this.bridge.waitFor('the stored Agent input override', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.inputs.some(input => input.key === key && input.stored) === true);
  await this.bridge.settings({ action: 'defaultAgentInput', key });
});

Then('the global Agent input {string} should equal {string}', async function (this: SapphireWorld, key: string, value: string) {
  await saved(this.testConfigPath, `literal input ${key}`, config => config.agent?.inputs?.[key], value);
});

Then('the profile {string} Agent input {string} should equal {string}', async function (this: SapphireWorld, profile: string, key: string, value: string) {
  await saved(path.join(this.fixtureRoot!, profile, 'config.json'), `literal input ${key}`, config => config.agent?.inputs?.[key], value);
});

Then('Agent input {string} should show {string} from {string}', async function (this: SapphireWorld, key: string, value: string, source: string) {
  await this.bridge.waitFor('the resolved input and source', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.inputs.some(input => input.key === key && input.value === value && input.hint === `${value} · ${source}`) === true);
});

Then('Agent input {string} should be absent', async function (this: SapphireWorld, key: string) {
  await this.bridge.waitFor('the removed or rejected Agent input', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.loaded === true && !state.settings.scope.agent.inputs.some(input => input.key === key));
});

Given('profile {string} has a prompt template with text:', function (this: SapphireWorld, profile: string, text: string) {
  fs.writeFileSync(path.join(this.fixtureRoot!, profile, 'preview.md'), text);
});

When('the user drafts Agent setting {string} as {string}', async function (this: SapphireWorld, key: string, value: string) {
  await this.bridge.settings({ action: 'selectSegment', key, label: 'Set' });
  await this.bridge.waitFor('the Agent text editor', () => this.bridge.state(), state =>
    state.settings?.scope?.fields?.some(field => field.key === key && field.segment === 'Set') === true);
  await this.bridge.settings({ action: 'type', key, value });
});

When('the user saves Agent setting {string}', async function (this: SapphireWorld, key: string) {
  await this.bridge.settings({ action: 'save', key });
});

Then('the saved Agent prompt preview should contain {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.waitFor('the saved prompt preview', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.previewText?.includes(text) === true && state.settings.scope.agent.previewError === null);
});

Then('the Agent prompt preview error should include {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.waitFor('the inline prompt preview error', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.previewError?.includes(text) === true);
});

Then('the Agent prompt preview should not be shown', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the global Agent card without a preview', () => this.bridge.state(), state =>
    state.settings?.scope?.agent?.canPreview === false);
});
