import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as path from 'node:path';
import type { AppState, HubMenu, HubState } from '../support/bridge';
import { SapphireWorld } from '../support/world';

// The phrases are Ember's (`project-ember/tests/steps/workspace-hub.steps.ts`); the hub is opened and driven with keys, as a user would, and clicked through the bridge's `hub` route.

const HUB = 'workspace-hub';
const CHEATSHEET = 'cheatsheet';
const DROPDOWN = 'dropdown-menu';

function hubOf(state: AppState): HubState {
  assert.ok(state.hub, `the Workspace Hub isn't open; window layers: ${JSON.stringify(state.windowLayers)}`);
  return state.hub;
}

async function waitForHub(world: SapphireWorld, what: string, done: (hub: HubState) => boolean): Promise<HubState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.hub !== null && done(s.hub));
  return hubOf(state);
}

/** Closes every window surface (the hub the launch opened, among others) with Escape, as a user would. */
export async function closeWindowSurfaces(world: SapphireWorld): Promise<void> {
  // A focused filter takes one Escape of its own.
  for (let i = 0; i < 5 && (await world.bridge.state()).windowLayers.length > 0; i++) {
    await world.bridge.press('Escape');
  }
  await world.bridge.waitFor('the window surfaces to close', () => world.bridge.state(), (s) => s.windowLayers.length === 0);
}

/**
 * Opens the hub with Ctrl+S and waits for its workspaces to load.
 * A hub already open (the launch's) is closed first, so that it loads the profiles the scenario has made since, as Ember's step reloads it.
 */
export async function openWorkspaceHub(world: SapphireWorld): Promise<HubState> {
  if ((await world.bridge.state()).windowLayers.length > 0) await closeWindowSurfaces(world);
  await world.bridge.press('s', ['control']);
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
  await typeText(this, text);
  await waitForHub(this, `the filter to read "${text}"`, (hub) => hub.filter === text);
});

/** Types into the focused field as key-downs, so the text goes through the field editor as a user's typing does; letters, digits and spaces only. */
export async function typeText(world: SapphireWorld, text: string): Promise<void> {
  for (const char of text) {
    if (char === ' ') await world.bridge.press('Space');
    else if (/[A-Z]/.test(char)) await world.bridge.press(char.toLowerCase(), ['shift']);
    else await world.bridge.press(char);
  }
}

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

async function openMenu(world: SapphireWorld, menu: HubMenu): Promise<void> {
  if (hubOf(await world.bridge.state()).menu?.kind !== menu) await world.bridge.hub({ action: 'toggleMenu', menu });
  await waitForHub(world, `the ${menu} dropdown to open`, (hub) => hub.menu?.kind === menu);
}

/** Opens a header dropdown and clicks one of its rows, as Ember's `clickBurgerMenuItem` and `switchProfile` do. */
async function clickMenuItem(world: SapphireWorld, menu: HubMenu, label: string): Promise<void> {
  await openMenu(world, menu);
  await world.bridge.hub({ action: 'menuItem', label });
  await waitForHub(world, `the ${menu} dropdown to close`, (hub) => hub.menu === null);
}

When('the user clicks the profile dropdown', async function (this: SapphireWorld) {
  await openMenu(this, 'profiles');
});

Then('the profile dropdown should not be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor(
    'the profile dropdown to close',
    () => this.bridge.state(),
    (s) => s.hub?.menu == null && !s.windowLayers.includes(DROPDOWN),
  );
});

Then('the profile dropdown should be labeled {string}', async function (this: SapphireWorld, name: string) {
  await waitForHub(this, `the profile dropdown to read "${name}"`, (hub) => hub.profileLabel === name);
});

When('the user switches to profile {string}', async function (this: SapphireWorld, name: string) {
  await clickMenuItem(this, 'profiles', name);
  const { profiles } = await this.bridge.state();
  const path = profiles.find((profile) => profile.name === name)?.path;
  await this.bridge.waitFor(`profile ${name} to be selected`, () => this.bridge.state(), (s) => s.selectedProfile === path && !!s.hub && !s.hub.loading);
});

When('the user clicks the save loading latencies button', async function (this: SapphireWorld) {
  await clickMenuItem(this, 'main', 'Save loading latencies');
  // The toast comes once the report is on disk.
  await waitForHub(this, 'the saved toast', (hub) => hub.toast?.includes('Loading latencies saved') ?? false);
});

Then('a profiling report file should exist in the diagnostics directory', function (this: SapphireWorld) {
  const diagnostics = path.join(this.fixtureRoot!, 'Work', 'diagnostics');
  const files = fs.readdirSync(diagnostics).filter((name) => name.startsWith('hub-profile-') && name.endsWith('.json'));
  assert.strictEqual(files.length, 1, `files in ${diagnostics}: ${files.join(', ')}`);
  const report = JSON.parse(fs.readFileSync(path.join(diagnostics, files[0]), 'utf-8'));
  assert.ok(report.timestamp, 'the report has a timestamp');
  assert.ok(report.phases?.totalLoad, 'the report has the totalLoad phase');
  assert.strictEqual(typeof report.workspaceCount, 'number');
});
