import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import {
  createTestFixtureRoot, createTestProfile, createTestRepo,
  createFakeTestWorkspaces, createRealTestWorkspaces,
  createInvalidWorkspaces, writeTestConfig,
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
    createFakeTestWorkspaces(profilePath, count, { active: count });
    writeTestConfig(this.testConfigPath!, [profilePath]);
  },
);

Given(
  'a profile {string} with {int} active and {int} inactive workspaces',
  async function (this: EmberWorld, name: string, active: number, inactive: number) {
    this.fixtureRoot = createTestFixtureRoot();
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createTestRepo(profilePath, 'test-repo');
    createFakeTestWorkspaces(profilePath, active + inactive, { active });
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
  'a profile {string} with {int} active workspace(s) with worktrees',
  async function (this: EmberWorld, name: string, count: number) {
    if (!this.fixtureRoot) {
      this.fixtureRoot = createTestFixtureRoot();
    }
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createTestRepo(profilePath, 'test-repo');
    createRealTestWorkspaces(profilePath, count, { active: count });

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
    const visible = await ui.hasElement('.keyboard-hint-bar');
    assert.strictEqual(visible, true, 'Hub legend should be visible');
  },
);

Then(
  'the hub search bar should be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.search-input');
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

// ---------------------------------------------------------------------------
// Workspace operations: Remove Task & Delete
// ---------------------------------------------------------------------------

Given(
  'a profile {string} with {int} inactive workspace(s) with worktrees',
  async function (this: EmberWorld, name: string, count: number) {
    if (!this.fixtureRoot) {
      this.fixtureRoot = createTestFixtureRoot();
    }
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createTestRepo(profilePath, 'test-repo');
    // Create workspaces as real worktrees (required for delete_workspace to work),
    // then remove task.json to make them inactive.
    createRealTestWorkspaces(profilePath, count, { active: count });
    for (let i = 1; i <= count; i++) {
      const taskPath = path.join(profilePath, 'workspaces', `ws-${i}`, 'task.json');
      if (fs.existsSync(taskPath)) {
        fs.unlinkSync(taskPath);
      }
    }

    let existingProfiles: string[] = [];
    if (fs.existsSync(this.testConfigPath!)) {
      const config = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf-8'));
      existingProfiles = config.profiles || [];
    }
    writeTestConfig(this.testConfigPath!, [...existingProfiles, profilePath]);
  },
);

Given(
  'a profile {string} with {int} invalid workspace(s)',
  async function (this: EmberWorld, name: string, count: number) {
    if (!this.fixtureRoot) {
      this.fixtureRoot = createTestFixtureRoot();
    }
    const profilePath = createTestProfile(this.fixtureRoot, name);
    createInvalidWorkspaces(profilePath, count);

    let existingProfiles: string[] = [];
    if (fs.existsSync(this.testConfigPath!)) {
      const config = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf-8'));
      existingProfiles = config.profiles || [];
    }
    writeTestConfig(this.testConfigPath!, [...existingProfiles, profilePath]);
  },
);

Given(
  'the workspace repos are clean and detached',
  async function (this: EmberWorld) {
    const wsPath = path.join(this.fixtureRoot!, 'Work', 'workspaces', 'ws-1');
    const repoDir = path.join(wsPath, 'test-repo');
    execSync('git checkout --detach', {
      cwd: repoDir,
      stdio: 'ignore',
    });
  },
);

Given(
  'workspace {string} has uncommitted changes in repo {string}',
  async function (this: EmberWorld, wsId: string, repoName: string) {
    const repoDir = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', wsId, repoName,
    );
    fs.writeFileSync(path.join(repoDir, 'dirty-file.txt'), 'uncommitted change\n');
  },
);

When(
  'the user clicks the "Remove Task" button',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    // Ensure repo statuses are loaded before clicking so the hub knows
    // whether repos are clean/detached (determines confirmation dialog).
    await waitForDetailRepoTable(this.driver);
    await ui.waitForElement('.ws-remove-task-btn');
    await ui.click('.ws-remove-task-btn');
  },
);

When(
  'the user clicks the "Delete" button',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.ws-delete-btn');
    await ui.click('.ws-delete-btn');
  },
);

When(
  'the user confirms the dialog',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.confirmation-dialog-confirm', 3_000);
    await ui.click('.confirmation-dialog-confirm');
    // Wait for the dialog to be dismissed
    await this.driver.pollFor(
      `return document.querySelector('.confirmation-dialog') === null`,
      true,
      5_000,
    );
  },
);

When(
  'the user cancels the dialog',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.confirmation-dialog-cancel', 3_000);
    await ui.click('.confirmation-dialog-cancel');
    // Wait for the dialog to be dismissed
    await this.driver.pollFor(
      `return document.querySelector('.confirmation-dialog') === null`,
      true,
      5_000,
    );
  },
);

Then(
  'a confirmation dialog should be visible',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.confirmation-dialog', 3_000);
  },
);

Then(
  'no confirmation dialog should appear',
  async function (this: EmberWorld) {
    // Short delay to give a dialog time to appear if it was going to
    await new Promise((r) => setTimeout(r, 500));
    const ui = new UIDriver(this.driver);
    const visible = await ui.hasElement('.confirmation-dialog');
    assert.strictEqual(visible, false, 'Confirmation dialog should not be visible');
  },
);

Then(
  'the task.json should no longer exist',
  async function (this: EmberWorld) {
    const taskPath = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1', 'task.json',
    );
    // Poll briefly to allow the backend to finish removing the file
    const deadline = Date.now() + 5_000;
    while (fs.existsSync(taskPath) && Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 100));
    }
    assert.strictEqual(
      fs.existsSync(taskPath), false,
      'task.json should be removed after task removal',
    );
  },
);

Then(
  'the task.json should still exist',
  async function (this: EmberWorld) {
    const taskPath = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1', 'task.json',
    );
    assert.strictEqual(
      fs.existsSync(taskPath), true,
      'task.json should still exist after cancellation',
    );
  },
);

Then(
  'the workspace repos should be clean and detached',
  async function (this: EmberWorld) {
    const repoDir = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1', 'test-repo',
    );
    const status = execSync('git status --porcelain', {
      cwd: repoDir,
      encoding: 'utf-8',
    }).trim();
    assert.strictEqual(status, '', 'Repo should have no uncommitted changes');

    const branch = execSync('git rev-parse --abbrev-ref HEAD', {
      cwd: repoDir,
      encoding: 'utf-8',
    }).trim();
    assert.strictEqual(branch, 'HEAD', 'Repo should be in detached HEAD state');
  },
);

Then(
  'the workspace should still have uncommitted changes',
  async function (this: EmberWorld) {
    const repoDir = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1', 'test-repo',
    );
    const status = execSync('git status --porcelain', {
      cwd: repoDir,
      encoding: 'utf-8',
    }).trim();
    assert.ok(status.length > 0, 'Repo should still have uncommitted changes');
  },
);

Then(
  'the workspace directory should not exist',
  async function (this: EmberWorld) {
    const wsPath = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1',
    );
    // Poll to allow the backend to finish deleting the directory
    const deadline = Date.now() + 5_000;
    while (fs.existsSync(wsPath) && Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 100));
    }
    assert.strictEqual(
      fs.existsSync(wsPath), false,
      `Workspace directory should not exist: ${wsPath}`,
    );
  },
);

Then(
  'the workspace directory should still exist',
  async function (this: EmberWorld) {
    const wsPath = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1',
    );
    assert.strictEqual(
      fs.existsSync(wsPath), true,
      `Workspace directory should still exist: ${wsPath}`,
    );
  },
);

Then(
  'the worktree directory should still exist',
  async function (this: EmberWorld) {
    const worktreePath = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', 'ws-1', 'test-repo',
    );
    assert.strictEqual(
      fs.existsSync(worktreePath), true,
      `Worktree directory should still exist: ${worktreePath}`,
    );
  },
);

Then(
  'the worktree should still be registered in the source repo',
  async function (this: EmberWorld) {
    const sourceRepo = path.join(this.fixtureRoot!, 'Work', 'repos', 'test-repo');
    const output = execSync('git worktree list', {
      cwd: sourceRepo,
      encoding: 'utf-8',
    });
    assert.ok(
      output.includes('workspaces/ws-1/test-repo'),
      `Worktree should be registered in source repo. git worktree list output: ${output}`,
    );
  },
);

Then(
  'the worktree should not be registered in the source repo',
  async function (this: EmberWorld) {
    const sourceRepo = path.join(this.fixtureRoot!, 'Work', 'repos', 'test-repo');
    const output = execSync('git worktree list', {
      cwd: sourceRepo,
      encoding: 'utf-8',
    });
    assert.ok(
      !output.includes('workspaces/ws-1/test-repo'),
      `Worktree should not be registered in source repo. git worktree list output: ${output}`,
    );
  },
);

Then(
  'the source repo should still exist',
  async function (this: EmberWorld) {
    const repoPath = path.join(this.fixtureRoot!, 'Work', 'repos', 'test-repo');
    assert.strictEqual(
      fs.existsSync(repoPath), true,
      `Source repo should still exist: ${repoPath}`,
    );
  },
);

Then(
  'the workspace list should show {int} item(s)',
  async function (this: EmberWorld, expected: number) {
    await waitForWorkspaceItems(this.driver, expected);
    const count = await getWorkspaceItemCount(this.driver);
    assert.strictEqual(count, expected);
  },
);

// ---------------------------------------------------------------------------
// New Task form: keyboard handling
// ---------------------------------------------------------------------------

Given(
  'the New Task form is open',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.ws-new-btn');
    await ui.click('.ws-new-btn');
    await this.driver.pollFor(
      `return document.querySelector('.ws-new-form') !== null`,
      true,
      5_000,
    );
  },
);

When(
  'the user types {string} into the focused task title input',
  async function (this: EmberWorld, key: string) {
    const ui = new UIDriver(this.driver);
    this.lastKeyDefaultPrevented = await ui.dispatchKeyToFocused('.ws-new-form-input', key);
  },
);

Then(
  'the New Task form should still be visible',
  async function (this: EmberWorld) {
    await this.driver.pollFor(
      `return document.querySelector('.ws-new-form') !== null`,
      true,
      3_000,
    );
  },
);

Then(
  'the keypress should not have been intercepted',
  function (this: EmberWorld) {
    assert.strictEqual(this.lastKeyDefaultPrevented, false);
  },
);
