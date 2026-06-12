import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { reloadWsHub, ensureWsHubClosed, waitForWorkspaceItems, getFocusedWorkspaceId } from '../support/ws-hub-helper';
import { openCommandPalette } from '../support/command-palette-helper';

const DIALOG_SELECTOR = '.text-input-dialog';

function workspacePath(world: EmberWorld, wsId: string): string {
  return path.join(world.fixtureRoot!, 'Work', 'workspaces', wsId);
}

function readTaskJson(world: EmberWorld, wsId: string): { uid: string; title: string } {
  const taskPath = path.join(workspacePath(world, wsId), 'task.json');
  return JSON.parse(fs.readFileSync(taskPath, 'utf-8'));
}

Given(
  'a session is opened for workspace {string} from the Hub',
  async function (this: EmberWorld, wsId: string) {
    await reloadWsHub(this.driver);
    await waitForWorkspaceItems(this.driver, 1);
    const focusedId = await getFocusedWorkspaceId(this.driver);
    assert.strictEqual(focusedId, wsId, `expected workspace ${wsId} to be focused in the Hub`);

    const ui = new UIDriver(this.driver);
    await ui.pressKey('Enter');

    // Opening a workspace dismisses the Hub and starts an (unawaited) session creation; block until the PTY is connected so later steps see a settled state.
    await this.driver.pollFor(
      'return window.__test.overlayManager.isTabCoveringOverlayActive()',
      false,
      5_000,
    );
    await this.driver.pollFor(
      'return window.__test.sessionManager.getActiveSession()?.getPtyId() != null',
      true,
      30_000,
    );
  },
);

When(
  'the user runs the {string} palette command',
  async function (this: EmberWorld, commandTitle: string) {
    await ensureWsHubClosed(this.driver);
    await openCommandPalette(this.driver);

    const ui = new UIDriver(this.driver);
    await ui.pressKey('f');
    await ui.typeIntoInput('.search-input-field', commandTitle);
    await this.driver.pollFor(
      `return document.querySelectorAll('.command-palette-item').length`,
      1,
      3_000,
    );
    await ui.pressKey('Enter');

    // Wait for the palette to close (every command dismisses it before acting)
    await this.driver.pollFor(
      `return document.querySelector('.command-palette') !== null`,
      false,
      3_000,
    );
  },
);

Then('the repurpose dialog should be visible', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.waitForElement(DIALOG_SELECTOR, 5_000);
});

Then('no repurpose dialog should appear', async function (this: EmberWorld) {
  // Short delay to give a dialog time to appear if it was going to
  await new Promise((r) => setTimeout(r, 500));
  const ui = new UIDriver(this.driver);
  const visible = await ui.hasElement(DIALOG_SELECTOR);
  assert.strictEqual(visible, false, 'Repurpose dialog should not be visible');
});

Then(
  'the repurpose dialog input should be pre-filled with {string}',
  async function (this: EmberWorld, expected: string) {
    const value = await this.driver.eval(
      `return document.querySelector('${DIALOG_SELECTOR}-input')?.value ?? null`,
    );
    assert.strictEqual(value, expected);
  },
);

When(
  'the user replaces the repurpose dialog input with {string}',
  async function (this: EmberWorld, text: string) {
    const ui = new UIDriver(this.driver);
    await ui.typeIntoInput(`${DIALOG_SELECTOR}-input`, text);
  },
);

When('the user confirms the repurpose dialog', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.click(`${DIALOG_SELECTOR}-confirm`);
  await this.driver.pollFor(
    `return document.querySelector('${DIALOG_SELECTOR}') === null`,
    true,
    5_000,
  );
});

When('the user cancels the repurpose dialog', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.click(`${DIALOG_SELECTOR}-cancel`);
  await this.driver.pollFor(
    `return document.querySelector('${DIALOG_SELECTOR}') === null`,
    true,
    5_000,
  );
});

Given(
  'the current task uid of workspace {string} is noted',
  function (this: EmberWorld, wsId: string) {
    this.originalTaskUid = readTaskJson(this, wsId).uid;
    assert.ok(this.originalTaskUid, 'task.json should have a uid');
  },
);

Then(
  'the task uid of workspace {string} should have changed',
  async function (this: EmberWorld, wsId: string) {
    // Poll briefly to allow the backend to finish writing the new task.json
    const deadline = Date.now() + 5_000;
    while (readTaskJson(this, wsId).uid === this.originalTaskUid && Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 100));
    }
    assert.notStrictEqual(
      readTaskJson(this, wsId).uid, this.originalTaskUid,
      'task uid should have been replaced',
    );
  },
);

Then(
  'the task uid of workspace {string} should be unchanged',
  async function (this: EmberWorld, wsId: string) {
    // Short delay to catch a late write if one was going to happen
    await new Promise((r) => setTimeout(r, 500));
    assert.strictEqual(readTaskJson(this, wsId).uid, this.originalTaskUid);
  },
);

Then(
  'the task.json title of workspace {string} should be {string}',
  async function (this: EmberWorld, wsId: string, expected: string) {
    // Poll briefly to allow the backend to finish writing the new task.json
    const deadline = Date.now() + 5_000;
    while (readTaskJson(this, wsId).title !== expected && Date.now() < deadline) {
      await new Promise((r) => setTimeout(r, 100));
    }
    assert.strictEqual(readTaskJson(this, wsId).title, expected);
  },
);

Given(
  'repo {string} of workspace {string} is on branch {string}',
  function (this: EmberWorld, repoName: string, wsId: string, branch: string) {
    const worktree = path.join(workspacePath(this, wsId), repoName);
    execSync(`git checkout -b ${JSON.stringify(branch)}`, {
      cwd: worktree,
      stdio: 'ignore',
    });
  },
);
