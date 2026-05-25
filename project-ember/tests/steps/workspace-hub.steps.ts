import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import {
  createTestFixtureRoot, createTestProfile, createTestRepo,
  createTestWorkspaces, createTestWorkspacesWithRealRepos, writeTestConfig,
} from '../support/fixture-helper';
import {
  reloadWsHub, getFocusedWorkspaceId,
  getFocusedWorkspaceTitle, getWorkspaceItemCount, getActiveCategoryFilter,
  waitForWorkspaceItems, filterWorkspaces, switchProfile, waitForDetailRepoTable,
  startWsHubLoad,
} from '../support/ws-hub-helper';

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
  'the Workspace Hub is open',
  async function (this: EmberWorld) {
    await reloadWsHub(this.driver);
    // Validate: hub element should exist in DOM
    const hubExists = await this.driver.eval(
      `return document.querySelector('.ws-hub') !== null`,
    );
    assert.strictEqual(hubExists, true, 'Workspace Hub should be visible after reload');
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
  'the Workspace Hub should show {int} workspace items',
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
  'the Workspace Hub should be visible',
  async function (this: EmberWorld) {
    const visible = await this.driver.eval(
      'return window.__test.overlayManager.isTabCoveringOverlayActive()',
    );
    assert.strictEqual(visible, true);
  },
);

Then(
  'the Workspace Hub should not be visible',
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

Given(
  'a profile {string} with {int} active workspaces and real repos',
  async function (this: EmberWorld, name: string, count: number) {
    if (!this.fixtureRoot) {
      this.fixtureRoot = createTestFixtureRoot();
    }
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createTestRepo(profilePath, 'test-repo');
    createTestWorkspacesWithRealRepos(profilePath, count, { active: count });

    let existingProfiles: string[] = [];
    if (fs.existsSync(this.testConfigPath!)) {
      const config = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf-8'));
      existingProfiles = config.profiles || [];
    }
    writeTestConfig(this.testConfigPath!, [...existingProfiles, profilePath]);

    // Validate setup: config should contain this profile
    const written = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf-8'));
    assert.ok(
      written.profiles.length > 0,
      `Config should have profiles after setup, got: ${JSON.stringify(written.profiles)}`,
    );
  },
);

When(
  'the user switches to profile {string}',
  async function (this: EmberWorld, profileName: string) {
    await switchProfile(this.driver, profileName);
  },
);

Then(
  'the detail panel should show repo status',
  async function (this: EmberWorld) {
    await waitForDetailRepoTable(this.driver);
  },
);

Given(
  'the Workspace Hub is loading',
  async function (this: EmberWorld) {
    await startWsHubLoad(this.driver);
  },
);

Then(
  'the hub header should be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.ws-header');
    assert.strictEqual(visible, true, 'Hub header should be visible');
  },
);

Then(
  'the hub legend should be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.ws-legend');
    assert.strictEqual(visible, true, 'Hub legend should be visible');
  },
);

Then(
  'the hub search bar should be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.ws-search');
    assert.strictEqual(visible, true, 'Hub search bar should be visible');
  },
);

When(
  'the user clicks the save loading latencies button',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.ws-profiling-btn');
    await ui.click('.ws-profiling-btn');
    await this.driver.pollFor(
      `return document.querySelector('.ws-profiling-saved') !== null`,
      true,
      5_000,
    );
  },
);

Then(
  'a profiling report file should exist in the diagnostics directory',
  async function (this: EmberWorld) {
    const profilePath = path.join(this.fixtureRoot!, 'Work');
    const diagnosticsDir = path.join(profilePath, 'diagnostics');
    assert.ok(fs.existsSync(diagnosticsDir), `Diagnostics directory should exist at ${diagnosticsDir}`);

    const files = fs.readdirSync(diagnosticsDir).filter((f) => f.startsWith('hub-profile-') && f.endsWith('.json'));
    assert.ok(files.length > 0, 'At least one profiling report file should exist');

    const reportContent = fs.readFileSync(path.join(diagnosticsDir, files[0]), 'utf-8');
    const report = JSON.parse(reportContent);
    assert.ok(report.timestamp, 'Report should have a timestamp');
    assert.ok(report.phases, 'Report should have phases');
    assert.ok(report.phases.totalLoad, 'Report should have totalLoad phase');
    assert.strictEqual(typeof report.workspaceCount, 'number', 'Report should have workspaceCount');
  },
);

