import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';

When('the user globally pauses Devora', async function (this: SapphireWorld) {
  await this.bridge.dispatch({ command: 'setGlobalPaused', paused: true });
});

Then('Devora is globally paused', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the global hold', () => this.bridge.state(), state => state.globalPaused);
});
Then('Devora is globally running', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the released global hold', () => this.bridge.state(), state => !state.globalPaused);
});
Then('the agent {string} has a paused tab indicator', async function (this: SapphireWorld, name: string) {
  await this.bridge.waitFor('the paused tab', () => this.bridge.state(), state => state.tabs.some(tab => tab.title === name && tab.agentStatus === 'paused'));
});
When('the user attempts to send during Global Pause', async function (this: SapphireWorld) {
  const chat = await this.bridge.waitFor('the held prompt', () => this.bridge.agentChat(), chat => chat.globallyPaused && !chat.canSend);
  await this.bridge.chatAction('send');
  assert.equal((await this.bridge.agentChat()).draft, chat.draft);
});
Then('the global pause send hint is shown', async function (this: SapphireWorld) {
  const chat = await this.bridge.agentChat();
  assert.equal(chat.globallyPaused, true);
  assert.equal(chat.canSend, false);
  assert.equal(chat.pauseWidget.label, 'Devora paused');
  assert.equal(chat.pauseWidget.tooltip, 'Resume Devora to send');
});
When('the user toggles Global Pause from the palette', async function (this: SapphireWorld) {
  const state = await this.bridge.state();
  const title = state.globalPaused ? 'Resume Devora' : 'Pause Devora';
  await this.bridge.palette('shiftShift');
  await this.bridge.waitFor('the global pause palette entry', () => this.bridge.state(), state => state.palette?.items.some(item => item.title === title) === true);
  await this.bridge.palette('filter', title);
  await this.bridge.press('Return');
});

const launches = new WeakMap<SapphireWorld, number>();
function launchCount(world: SapphireWorld): number {
  const file = path.join(path.dirname(world.testConfigPath), 'fake-claude.jsonl');
  return fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map(line => JSON.parse(line)).filter(record => record.kind === 'start').length;
}
When('the current agent CLI launch count is recorded', function (this: SapphireWorld) { launches.set(this, launchCount(this)); });
Then('the agent CLI launch count is unchanged', function (this: SapphireWorld) { assert.equal(launchCount(this), launches.get(this)); });

When('the user keeps this agent paused through Global Pause', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the Keep paused action', () => this.bridge.agentChat(), s => s.pauseWidget.keepPaused);
  await this.bridge.chatAction('pause');
  await this.bridge.waitFor('the individual hold', () => this.bridge.agentChat(), s => !s.globallyHeld && s.pauseWidget.keptPaused);
});
Then('the pause widget shows {string}', async function (this: SapphireWorld, label: string) {
  await this.bridge.waitFor(`pause widget ${label}`, () => this.bridge.agentChat(), s => s.pauseWidget.label === label);
});
Then('the agent resume action is available', async function (this: SapphireWorld) {
  const chat = await this.bridge.agentChat();
  assert.equal(chat.pauseWidget.label, 'Resume');
  assert.equal(chat.pauseWidget.enabled, true);
});
