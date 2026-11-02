import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';
import { clickMenuItem } from './workspace-hub.steps';

When('the user opens Health from the Workspace Hub menu', async function (this: SapphireWorld) {
  await clickMenuItem(this, 'main', 'Health');
});
When('the Health Hub finishes loading', async function (this: SapphireWorld) {
  const state = await this.bridge.waitFor('the Health Hub to finish', () => this.bridge.state(), (s) => s.health !== null && !s.health.loading);
  assert.equal(state.health?.error, null, `Health Hub failed: ${state.health?.error}`);
});
Then('the Health Hub should be visible', async function (this: SapphireWorld) {
  assert.ok((await this.bridge.state()).windowLayers.includes('health-hub'));
});
Then('the Health Hub should show the {string} section', async function (this: SapphireWorld, title: string) {
  assert.ok((await this.bridge.state()).health?.sections.some((section) => section.title === title));
});
Then('the Health Hub should report the config file as found', async function (this: SapphireWorld) {
  assert.ok((await this.bridge.state()).health?.sections.some((section) => section.items.some((item) => item.name === 'config' && item.message === 'found')));
});
Then('the Health Hub should show a version', async function (this: SapphireWorld) {
  assert.ok((await this.bridge.state()).health?.version);
});

When('the health fixture version changes to {string}', function (this: SapphireWorld, version: string) {
  const fixture = path.join(this.scenarioDir, 'health-report.json');
  const report = JSON.parse(fs.readFileSync(fixture, 'utf8'));
  report.version = version;
  fs.writeFileSync(fixture, JSON.stringify(report));
});

Then('the Health Hub should eventually show version {string}', async function (this: SapphireWorld, version: string) {
  await this.bridge.waitFor('the updated Health report', () => this.bridge.state(), (s) => s.health?.version === version && !s.health.loading);
});
