import { Given, When, Then } from '@cucumber/cucumber';
import type { AppState } from '../support/bridge';
import { SapphireWorld } from '../support/world';

// The phrases are Ember's, so that the scenarios in `acceptance/features/` run on both apps; a session is a tab.

Given('Sapphire is running', async function (this: SapphireWorld) {
  // The launch hook only returns once the app has finished launching.
});

Given('Devora is running with no sessions open', async function (this: SapphireWorld) {
  for (const tab of (await this.bridge.state()).tabs) {
    await this.bridge.dispatch({ command: 'closeTab', id: tab.id });
  }
  await waitForSessionCount(this, 0);
});

Given('{int} sessions exist', async function (this: SapphireWorld, count: number) {
  for (let i = 0; i < count; i++) await openShell(this);
});

When('a new session is created', async function (this: SapphireWorld) {
  await openShell(this);
});

Then('there should be {int} session(s)', async function (this: SapphireWorld, count: number) {
  await waitForSessionCount(this, count);
});

When('the user switches to the previous session', async function (this: SapphireWorld) {
  const { tabs, activeTab } = await this.bridge.state();
  const index = tabs.findIndex((tab) => tab.id === activeTab);
  const previous = tabs[(index - 1 + tabs.length) % tabs.length];
  await this.bridge.dispatch({ command: 'activateTab', id: previous.id });
  await waitForActiveTab(this, previous.id);
});

Then('the first session should be active', async function (this: SapphireWorld) {
  const first = (await this.bridge.state()).tabs[0];
  await waitForActiveTab(this, first.id);
});

When('the active session is closed', async function (this: SapphireWorld) {
  const { activeTab } = await this.bridge.state();
  await this.bridge.dispatch({ command: 'closeTab', id: activeTab! });
  await this.bridge.waitFor(`tab ${activeTab} to close`, () => this.bridge.state(), (s) => !s.tabs.some((tab) => tab.id === activeTab));
});

When('{string} is typed in the terminal', async function (this: SapphireWorld, line: string) {
  // The terminal starts once its tab is on screen; typing before that has nowhere to go.
  await this.bridge.waitFor('the active terminal to run', () => this.bridge.terminalText(), () => true);
  await this.bridge.typeLine(line);
});

Then('the terminal should contain {string}', async function (this: SapphireWorld, expected: string) {
  await this.bridge.waitFor(`the terminal to contain "${expected}"`, () => this.bridge.terminalText(), (text) => text.includes(expected));
});

Then("the active session's terminal should have the keyboard focus", async function (this: SapphireWorld) {
  await this.bridge.waitFor(
    'the active tab to be the only focused one',
    () => this.bridge.state(),
    (s) => s.tabs.every((tab) => tab.focused === (tab.id === s.activeTab)),
  );
});

/** Opens a shell tab and waits until it is the active one, as clicking `+` leaves it. */
async function openShell(world: SapphireWorld): Promise<void> {
  const before = new Set((await world.bridge.state()).tabs.map((tab) => tab.id));
  await world.bridge.dispatch({ command: 'openShell' });
  await world.bridge.waitFor(
    'the new shell tab to be active',
    () => world.bridge.state(),
    (s) => s.activeTab !== null && !before.has(s.activeTab),
  );
}

function waitForSessionCount(world: SapphireWorld, count: number): Promise<AppState> {
  return world.bridge.waitFor(`${count} tab(s)`, () => world.bridge.state(), (s) => s.tabs.length === count);
}

function waitForActiveTab(world: SapphireWorld, id: number): Promise<AppState> {
  return world.bridge.waitFor(`tab ${id} to be active`, () => world.bridge.state(), (s) => s.activeTab === id);
}
