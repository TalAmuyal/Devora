import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import {
  createInvalidWorkspaces,
  createTestRepo,
  createTestWorkspaces,
} from '../../../project-ember/tests/support/fixture-helper';
import { TEST_REPO_NAME, addProfile } from './workspace.steps';
import { SapphireWorld } from '../support/world';

// The phrases are Ember's (`project-ember/tests/steps/workspace-hub.steps.ts`); a click is the bridge's `hub` or `dialog` route, which runs what the button runs.

function workspaceDir(world: SapphireWorld, id = 'ws-1'): string {
  return path.join(world.fixtureRoot!, 'Work', 'workspaces', id);
}

function sourceRepo(world: SapphireWorld): string {
  return path.join(world.fixtureRoot!, 'Work', 'repos', TEST_REPO_NAME);
}

function git(dir: string, command: string): string {
  return execSync(`git ${command}`, { cwd: dir, encoding: 'utf-8' });
}

/** Polls the file system, where the hub's operations land after their core call returns. */
async function eventually(what: string, check: () => boolean): Promise<void> {
  const deadline = Date.now() + 5_000;
  while (!check()) {
    if (Date.now() > deadline) throw new Error(`Timed out waiting for ${what}`);
    await new Promise((r) => setTimeout(r, 100));
  }
}

Given(
  'a profile {string} with {int} inactive workspace(s) with worktrees',
  function (this: SapphireWorld, name: string, count: number) {
    addProfile(this, name, (profilePath) => {
      createTestRepo(profilePath, TEST_REPO_NAME);
      createTestWorkspaces(profilePath, count, { active: 0 }, TEST_REPO_NAME);
    });
  },
);

Given('a profile {string} with {int} invalid workspace(s)', function (this: SapphireWorld, name: string, count: number) {
  addProfile(this, name, (profilePath) => createInvalidWorkspaces(profilePath, count));
});

Given('the workspace repos are clean and detached', function (this: SapphireWorld) {
  git(path.join(workspaceDir(this), TEST_REPO_NAME), 'checkout -q --detach');
});

Given(
  'workspace {string} has uncommitted changes in repo {string}',
  function (this: SapphireWorld, id: string, repo: string) {
    fs.writeFileSync(path.join(workspaceDir(this, id), repo, 'dirty-file.txt'), 'uncommitted change\n');
  },
);

When('the worktree repo of workspace {string} becomes corrupted', function (this: SapphireWorld, id: string) {
  // Without its .git link, every git command in the worktree fails, as after an out-of-band breakage once the hub has a healthy status.
  fs.rmSync(path.join(workspaceDir(this, id), TEST_REPO_NAME, '.git'), { recursive: true, force: true });
});

When('the user clicks the {string} button', async function (this: SapphireWorld, label: string) {
  const actions: Record<string, 'removeTask' | 'delete'> = { 'Remove Task': 'removeTask', Delete: 'delete' };
  const action = actions[label];
  if (!action) throw new Error(`No "${label}" button in the hub`);
  // As Ember's step does: the decision to confirm needs the focused workspace's status.
  if (action === 'removeTask') {
    await this.bridge.waitFor('the focused workspace\'s status', () => this.bridge.state(), (s) => s.hub?.detail?.repos != null);
  }
  await this.bridge.hub({ action });
});

Then('a confirmation dialog should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('a confirmation dialog', () => this.bridge.state(), (s) => s.dialog !== null);
});

Then('no confirmation dialog should appear', async function (this: SapphireWorld) {
  // Long enough for a dialog that was going to open, as Ember's step waits.
  await new Promise((r) => setTimeout(r, 500));
  const { dialog, windowLayers } = await this.bridge.state();
  assert.strictEqual(dialog, null, `a dialog is open; window layers: ${JSON.stringify(windowLayers)}`);
});

When('the user confirms the dialog', async function (this: SapphireWorld) {
  await this.bridge.dialog('confirm');
  await this.bridge.waitFor('the dialog to close', () => this.bridge.state(), (s) => s.dialog === null);
});

When('the user cancels the dialog', async function (this: SapphireWorld) {
  await this.bridge.dialog('cancel');
  await this.bridge.waitFor('the dialog to close', () => this.bridge.state(), (s) => s.dialog === null);
});

Then('the task.json should no longer exist', async function (this: SapphireWorld) {
  await eventually('task.json to be removed', () => !fs.existsSync(path.join(workspaceDir(this), 'task.json')));
});

Then('the task.json should still exist', function (this: SapphireWorld) {
  assert.ok(fs.existsSync(path.join(workspaceDir(this), 'task.json')));
});

Then('the workspace repos should be clean and detached', function (this: SapphireWorld) {
  const repo = path.join(workspaceDir(this), TEST_REPO_NAME);
  assert.strictEqual(git(repo, 'status --porcelain').trim(), '');
  assert.strictEqual(git(repo, 'rev-parse --abbrev-ref HEAD').trim(), 'HEAD');
});

Then('the workspace should still have uncommitted changes', function (this: SapphireWorld) {
  assert.ok(git(path.join(workspaceDir(this), TEST_REPO_NAME), 'status --porcelain').trim().length > 0);
});

Then('the workspace directory should not exist', async function (this: SapphireWorld) {
  await eventually('the workspace directory to be deleted', () => !fs.existsSync(workspaceDir(this)));
});

Then('the workspace directory should still exist', function (this: SapphireWorld) {
  assert.ok(fs.existsSync(workspaceDir(this)));
});

Then('the worktree directory should still exist', function (this: SapphireWorld) {
  assert.ok(fs.existsSync(path.join(workspaceDir(this), TEST_REPO_NAME)));
});

Then('the worktree should still be registered in the source repo', function (this: SapphireWorld) {
  const worktrees = git(sourceRepo(this), 'worktree list');
  assert.ok(worktrees.includes(`workspaces/ws-1/${TEST_REPO_NAME}`), worktrees);
});

Then('the worktree should not be registered in the source repo', function (this: SapphireWorld) {
  const worktrees = git(sourceRepo(this), 'worktree list');
  assert.ok(!worktrees.includes(`workspaces/ws-1/${TEST_REPO_NAME}`), worktrees);
});

Then('the source repo should still exist', function (this: SapphireWorld) {
  assert.ok(fs.existsSync(sourceRepo(this)));
});

Then('the workspace list should show {int} item(s)', async function (this: SapphireWorld, count: number) {
  await this.bridge.waitFor(`${count} workspace items`, () => this.bridge.state(), (s) => s.hub?.items.length === count);
});
