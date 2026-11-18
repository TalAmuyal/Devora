import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { When, Then } from '@cucumber/cucumber';
import { exited, hasExited } from '../support/app';
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
  assert.equal(chat.pauseWidget.label, 'Paused');
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
  await this.bridge.setKeepPaused(true);
  await this.bridge.waitFor('the selected Keep paused switch', () => this.bridge.agentChat(), s => s.globallyHeld && s.pauseWidget.keptPaused);
});
When('the user lets this agent resume with Devora', async function (this: SapphireWorld) {
  await this.bridge.setKeepPaused(false);
  await this.bridge.waitFor('the cleared Keep paused switch', () => this.bridge.agentChat(), s => s.globallyHeld && !s.pauseWidget.keptPaused);
});
Then('the Keep paused switch is on', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the selected Keep paused switch', () => this.bridge.agentChat(), s => s.globallyHeld && s.pauseWidget.keptPaused);
});
Then('the pause widget shows {string}', async function (this: SapphireWorld, label: string) {
  await this.bridge.waitFor(`pause widget ${label}`, () => this.bridge.agentChat(), s => s.pauseWidget.label === label);
});
Then('the agent resume action is available', async function (this: SapphireWorld) {
  const chat = await this.bridge.agentChat();
  assert.equal(chat.pauseWidget.label, 'Resume');
  assert.equal(chat.pauseWidget.enabled, true);
});

const ACTIVITIES: Record<string, string> = {
  'running a turn': 'turn',
  'waiting for an answer': 'question',
};

When('Sapphire is asked to quit', async function (this: SapphireWorld) {
  await this.bridge.quit();
});
When('the user closes the window', async function (this: SapphireWorld) {
  await this.bridge.closeWindow();
});
Then('the agent {string} is running a turn', async function (this: SapphireWorld, name: string) {
  await this.bridge.waitFor('the running turn', () => this.bridge.state(), state => state.runningAgents.running.some(agent => agent.name === name && agent.activity === 'turn'));
});
Then('Sapphire waits to quit for {string}, which is {string}', async function (this: SapphireWorld, name: string, activity: string) {
  const expected = ACTIVITIES[activity];
  assert.ok(expected, `unknown activity "${activity}"`);
  await this.bridge.waitFor(`the quit waiting for ${name}`, () => this.bridge.state(), state =>
    state.quitPending && state.pauseProgress?.mode === 'quitting'
    && state.runningAgents.running.some(agent => agent.name === name && agent.activity === expected));
});
Then('the toolbar shows {string}', async function (this: SapphireWorld, indicator: string) {
  await this.bridge.waitFor(`the toolbar's "${indicator}"`, () => this.bridge.state(), state => state.pauseIndicator === indicator);
});
When('the user shows the agent {string} from the quit progress', async function (this: SapphireWorld, name: string) {
  await this.bridge.pauseProgressShow(name);
  await this.bridge.waitFor('the progress closed over the agent tab', () => this.bridge.state(), state =>
    state.pauseProgress === null && state.tabs.find(tab => tab.id === state.activeTab)?.title === name);
});
When('the user cancels the quit', async function (this: SapphireWorld) {
  await this.bridge.pauseProgressButton('Cancel Quit');
});
When('the user interrupts and quits', async function (this: SapphireWorld) {
  await this.bridge.pauseProgressButton('Interrupt and Quit', true);
});
Then('Sapphire quits by itself', async function (this: SapphireWorld) {
  assert.equal(await exited(this.app, 30_000), 0);
});
Then('Sapphire is still running with no quit pending', async function (this: SapphireWorld) {
  const state = await this.bridge.waitFor('the cancelled quit', () => this.bridge.state(), state => !state.quitPending && state.pauseProgress === null);
  assert.equal(hasExited(this.app), false);
  assert.equal(state.globalPaused, true, 'the hold stays after Cancel');
});
