import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import {
  createTestFixtureRoot, createTestProfile, createTestRepo,
  createTestWorkspaces, writeTestConfig,
} from '../support/fixture-helper';
import {
  reloadWsPanel, getFocusedWorkspaceId,
  getFocusedWorkspaceTitle, getWorkspaceItemCount, getActiveCategoryFilter,
  waitForWorkspaceItems, filterWorkspaces,
} from '../support/ws-panel-helper';

Given(
  'a profile {string} with {int} active workspaces',
  async function (this: EmberWorld, name: string, count: number) {
    this.fixtureRoot = createTestFixtureRoot();
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createTestRepo(profilePath, 'test-repo');
    createTestWorkspaces(profilePath, count, { active: count });
    writeTestConfig(this.testConfigPath!, [profilePath]);
  },
);

Given(
  'a profile {string} with {int} active and {int} inactive workspaces',
  async function (this: EmberWorld, name: string, active: number, inactive: number) {
    this.fixtureRoot = createTestFixtureRoot();
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createTestRepo(profilePath, 'test-repo');
    createTestWorkspaces(profilePath, active + inactive, { active });
    writeTestConfig(this.testConfigPath!, [profilePath]);
  },
);

Given(
  'the workspace-management panel is open',
  async function (this: EmberWorld) {
    await reloadWsPanel(this.driver);
  },
);

When(
  'the user presses {string}',
  async function (this: EmberWorld, key: string) {
    const ui = new UIDriver(this.driver);
    await ui.pressKey(key);
  },
);

When(
  'the user presses {string} {int} times',
  async function (this: EmberWorld, key: string, count: number) {
    const ui = new UIDriver(this.driver);
    await ui.pressKeyMultiple(key, count);
  },
);

When(
  'the user filters workspaces by {string}',
  async function (this: EmberWorld, text: string) {
    const ui = new UIDriver(this.driver);
    await filterWorkspaces(ui, this.driver, text);
  },
);

Then(
  'the workspace-management panel should show {int} workspace items',
  async function (this: EmberWorld, expected: number) {
    await waitForWorkspaceItems(this.driver, expected);
    const count = await getWorkspaceItemCount(this.driver);
    assert.strictEqual(count, expected);
  },
);

Then(
  'the focused workspace should be {string}',
  async function (this: EmberWorld, expected: string) {
    const id = await getFocusedWorkspaceId(this.driver);
    assert.strictEqual(id, expected);
  },
);

Then(
  'the focused workspace title should be {string}',
  async function (this: EmberWorld, expected: string) {
    const title = await getFocusedWorkspaceTitle(this.driver);
    assert.strictEqual(title, expected);
  },
);

Then(
  'the workspace-management panel should be visible',
  async function (this: EmberWorld) {
    const visible = await this.driver.eval(
      'return window.__test.overlayManager.isTabCoveringOverlayActive()',
    );
    assert.strictEqual(visible, true);
  },
);

Then(
  'the workspace-management panel should not be visible',
  async function (this: EmberWorld) {
    await this.driver.pollFor(
      'return window.__test.overlayManager.isTabCoveringOverlayActive()',
      false,
      3_000,
    );
  },
);

Then(
  'the active category should be {string}',
  async function (this: EmberWorld, expected: string) {
    const category = await getActiveCategoryFilter(this.driver);
    assert.strictEqual(category, expected);
  },
);

Then(
  'the cheatsheet should be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.ws-cheatsheet');
    assert.strictEqual(visible, true);
  },
);

Then(
  'the cheatsheet should not be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.ws-cheatsheet');
    assert.strictEqual(visible, false);
  },
);
