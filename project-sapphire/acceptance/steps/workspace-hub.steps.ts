import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import type { AppState, HubState } from '../support/bridge';
import { SapphireWorld } from '../support/world';

// The phrases are Ember's (`project-ember/tests/steps/workspace-hub.steps.ts`); the hub is opened and driven with keys, as a user would.

const HUB = 'workspace-hub';
const CHEATSHEET = 'cheatsheet';

function hubOf(state: AppState): HubState {
  assert.ok(state.hub, `the Workspace Hub isn't open; window layers: ${JSON.stringify(state.windowLayers)}`);
  return state.hub;
}

async function waitForHub(world: SapphireWorld, what: string, done: (hub: HubState) => boolean): Promise<HubState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.hub !== null && done(s.hub));
  return hubOf(state);
}

/** Opens the hub with Ctrl+S, unless it is open, and waits for its workspaces to load. */
export async function openWorkspaceHub(world: SapphireWorld): Promise<HubState> {
  if ((await world.bridge.state()).hub === null) await world.bridge.press('s', ['control']);
  return waitForHub(world, 'the Workspace Hub to load', (hub) => !hub.loading);
}

Given('the Workspace Hub is open', async function (this: SapphireWorld) {
  await openWorkspaceHub(this);
});

Then('the Workspace Hub should be visible', async function (this: SapphireWorld) {
  assert.ok((await this.bridge.state()).windowLayers.includes(HUB));
});

Then('the Workspace Hub overlay should be present', async function (this: SapphireWorld) {
  assert.ok((await this.bridge.state()).windowLayers.includes(HUB));
});

Then('the Workspace Hub should not be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the Workspace Hub to close', () => this.bridge.state(), (s) => !s.windowLayers.includes(HUB));
});

Then('the Workspace Hub should show {int} workspace items', async function (this: SapphireWorld, count: number) {
  await waitForHub(this, `${count} workspace items`, (hub) => !hub.loading && hub.items.length === count);
});

Then('the focused workspace should be {string}', async function (this: SapphireWorld, id: string) {
  await waitForHub(this, `${id} to be focused`, (hub) => hub.focused === id);
});

Then('the active category should be {string}', async function (this: SapphireWorld, category: string) {
  await waitForHub(this, `the ${category} category`, (hub) => hub.category === category);
});

Given('the category filter is set to {string}', async function (this: SapphireWorld, category: string) {
  const keys: Record<string, string> = { Active: '1', Inactive: '2', All: '3' };
  await this.bridge.press(keys[category]);
  await waitForHub(this, `the ${category} category`, (hub) => hub.category === category);
});

Then('the category filter should still be {string}', async function (this: SapphireWorld, category: string) {
  assert.strictEqual(hubOf(await this.bridge.state()).category, category);
});

When('the user filters workspaces by {string}', async function (this: SapphireWorld, text: string) {
  await this.bridge.press('f');
  await waitForHub(this, 'the filter to have the focus', (hub) => hub.filterFocused);
  // Typed into the field as key-downs, so the text goes through the field editor as a user's typing does.
  for (const char of text) {
    if (char === ' ') await this.bridge.press('Space');
    else if (/[A-Z]/.test(char)) await this.bridge.press(char.toLowerCase(), ['shift']);
    else await this.bridge.press(char);
  }
  await waitForHub(this, `the filter to read "${text}"`, (hub) => hub.filter === text);
});

Then('the detail panel should show repo status', async function (this: SapphireWorld) {
  await waitForHub(
    this,
    "the focused workspace's repo status",
    (hub) => hub.detail !== null && hub.detail.error === null && (hub.detail.repos?.length ?? 0) > 0,
  );
});

Then('the cheatsheet should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the cheatsheet to open', () => this.bridge.state(), (s) => s.windowLayers.at(-1) === CHEATSHEET);
});

Then('the cheatsheet should not be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the cheatsheet to close', () => this.bridge.state(), (s) => !s.windowLayers.includes(CHEATSHEET));
});

Then('the New Task dialog should not be open', async function (this: SapphireWorld) {
  assert.deepStrictEqual((await this.bridge.state()).windowLayers, [HUB, CHEATSHEET]);
});

Then('the cheatsheet should overflow its viewport', async function (this: SapphireWorld) {
  await waitForHub(
    this,
    'the cheatsheet to be taller than its viewport',
    (hub) => !!hub.cheatsheet && hub.cheatsheet.viewportHeight > 0 && hub.cheatsheet.contentHeight > hub.cheatsheet.viewportHeight,
  );
});

// The relative motions scroll smoothly; wait for the offset to settle, so that a following jump is measured from where it stopped.
Then('the cheatsheet scroll offset should be greater than 0', async function (this: SapphireWorld) {
  let previous = -1;
  await waitForHub(this, 'the cheatsheet to scroll down and settle', (hub) => {
    const offset = hub.cheatsheet?.offset ?? 0;
    const settled = offset > 0 && offset === previous;
    previous = offset;
    return settled;
  });
});

Then('the cheatsheet scroll offset should be 0', async function (this: SapphireWorld) {
  await waitForHub(this, 'the cheatsheet to scroll to the top', (hub) => hub.cheatsheet?.offset === 0);
});

Then('the refresh toast should read {string}', async function (this: SapphireWorld, text: string) {
  await waitForHub(this, `the toast to read "${text}"`, (hub) => hub.toast?.includes(text) ?? false);
});

Then('the refresh toast should disappear', async function (this: SapphireWorld) {
  await waitForHub(this, 'the toast to disappear', (hub) => hub.toast === null);
});
