import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { GIT_TEST_IDENTITY } from '../../../project-ember/tests/support/git-test-env';
import { createOriginBackedRepo } from '../../../project-ember/tests/support/task-creation-fixture';
import type { AddRepoState, AppState, CloneRepoState, NewTaskState } from '../support/bridge';
import { SapphireWorld } from '../support/world';
import { openWorkspaceHub, typeText } from './workspace-hub.steps';

// The phrases are Ember's (`project-ember/tests/steps/add-repo.steps.ts` and `clone-repo.steps.ts`).
// Add Repo opens from the Workspace Hub's detail panel on the active tab's workspace, where Ember runs its palette command; the dialogs' fields are typed into with keys (the URL through the bridge, since it has characters the bridge's keys lack), and clicked through the bridge's `add-repo` and `clone-repo` routes.

const GIT_ENV = { ...process.env, ...GIT_TEST_IDENTITY };

function git(args: string, cwd: string): string {
  return execSync(`git ${args}`, { cwd, env: GIT_ENV, stdio: ['ignore', 'pipe', 'ignore'] }).toString();
}

function profileDir(world: SapphireWorld): string {
  assert.ok(world.profilePath, 'no profile was made');
  return world.profilePath;
}

function bareRepoPath(world: SapphireWorld, repoName: string): string {
  return path.join(world.fixtureRoot!, `${repoName}.git`);
}

async function waitForAddRepo(world: SapphireWorld, what: string, done: (dialog: AddRepoState) => boolean): Promise<AddRepoState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.addRepo !== null && done(s.addRepo));
  return state.addRepo!;
}

async function waitForCloneRepo(world: SapphireWorld, what: string, done: (dialog: CloneRepoState) => boolean): Promise<CloneRepoState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.cloneRepo !== null && done(s.cloneRepo));
  return state.cloneRepo!;
}

async function waitForNewTask(world: SapphireWorld, what: string, done: (dialog: NewTaskState) => boolean): Promise<void> {
  await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.newTask !== null && done(s.newTask));
}

function activeTabTitle(state: AppState): string | undefined {
  return state.tabs.find((tab) => tab.id === state.activeTab)?.title;
}

// --- Fixtures -------------------------------------------------------------

Given('an additional origin-backed repo {string}', function (this: SapphireWorld, repoName: string) {
  createOriginBackedRepo(this.fixtureRoot!, profileDir(this), repoName);
});

// A standalone bare repo with one commit on main, outside every profile's repos/: the user clones it by its file:// URL.
Given('a bare repo {string} available to clone', function (this: SapphireWorld, repoName: string) {
  const bare = bareRepoPath(this, repoName);
  fs.mkdirSync(bare, { recursive: true });
  git('init --bare -b main', bare);
  const seed = path.join(this.fixtureRoot!, `${repoName}-seed`);
  git(`clone ${JSON.stringify(bare)} ${JSON.stringify(seed)}`, this.fixtureRoot!);
  fs.writeFileSync(path.join(seed, 'file.txt'), 'v1\n');
  git('add .', seed);
  git('commit -m c1', seed);
  git('push origin main', seed);
});

// --- Add Repo -------------------------------------------------------------

When("the user opens Add Repo for the active session's workspace", async function (this: SapphireWorld) {
  const title = activeTabTitle(await this.bridge.state());
  const hub = await openWorkspaceHub(this);
  const focused = hub.items.find((item) => item.id === hub.focused);
  assert.ok(focused && focused.title === title, `the hub focuses ${JSON.stringify(focused)}, not the active tab's workspace (${title})`);
  await this.bridge.hub({ action: 'addRepo' });
  await this.bridge.waitFor(
    'the Add Repo dialog with its filter focused',
    () => this.bridge.state(),
    (s) => s.addRepo?.focus === 'filter' && s.editingText,
  );
});

async function startAdding(world: SapphireWorld, repoName: string, postfix: string): Promise<void> {
  await world.bridge.addRepo({ action: 'selectRepo', name: repoName });
  await waitForAddRepo(world, `${repoName} to be selected`, (dialog) => dialog.repos.some((r) => r.name === repoName && r.selected));
  await submitWithPostfix(world, postfix);
}

/** Types the postfix into its field, the next in the dialog's Tab order after the filter, then clicks Add Repo. */
async function submitWithPostfix(world: SapphireWorld, postfix: string): Promise<void> {
  if (postfix) {
    await world.bridge.press('Tab');
    await waitForAddRepo(world, 'the postfix to have the focus', (dialog) => dialog.focus === 'postfix');
    await typeText(world, postfix);
    await waitForAddRepo(world, `the postfix to read "${postfix}"`, (dialog) => dialog.postfix === postfix);
  }
  await world.bridge.addRepo({ action: 'submit' });
}

async function waitForAddRepoToClose(world: SapphireWorld): Promise<void> {
  await world.bridge.waitFor('the Add Repo dialog to close', () => world.bridge.state(), (s) => s.addRepo === null, 30_000);
}

When('the user adds repo {string} with postfix {string}', async function (this: SapphireWorld, repoName: string, postfix: string) {
  await startAdding(this, repoName, postfix);
  await waitForAddRepoToClose(this);
});

When('the user starts adding repo {string} with postfix {string}', async function (this: SapphireWorld, repoName: string, postfix: string) {
  await startAdding(this, repoName, postfix);
});

When('the user cancels adding the repo once it is preparing', async function (this: SapphireWorld) {
  await waitForAddRepo(
    this,
    'the add to be preparing',
    (dialog) => dialog.progress?.steps.some((step) => step.label.startsWith('Preparing')) ?? false,
  );
  await this.bridge.addRepo({ action: 'progressAction' });
});

Then('the Add Repo dialog should close', async function (this: SapphireWorld) {
  await waitForAddRepoToClose(this);
});

When('the user filters the add-repo list by {string}', async function (this: SapphireWorld, text: string) {
  await typeText(this, text);
  await waitForAddRepo(this, `the filter to read "${text}"`, (dialog) => dialog.filter === text);
});

Then('the add-repo list should show {int} repo(s)', async function (this: SapphireWorld, count: number) {
  await waitForAddRepo(this, `${count} repos in the Add Repo dialog`, (dialog) => dialog.repos.length === count);
});

When('the user submits the add-repo dialog with postfix {string}', async function (this: SapphireWorld, postfix: string) {
  // The single visible repo is selected, so Add Repo adds it without an explicit pick.
  await submitWithPostfix(this, postfix);
  await waitForAddRepoToClose(this);
});

Then('the worktree {string} should exist in workspace {string}', function (this: SapphireWorld, worktreeName: string, wsId: string) {
  const worktree = path.join(profileDir(this), 'workspaces', wsId, worktreeName);
  assert.ok(fs.existsSync(path.join(worktree, '.git')), `worktree should exist: ${worktree}`);
  // Checked out from origin/main, so the committed file is present.
  assert.ok(fs.existsSync(path.join(worktree, 'file.txt')), `worktree should be checked out: ${worktree}`);
  assert.match(git('rev-parse HEAD', worktree).trim(), /^[0-9a-f]{40}$/, 'worktree HEAD should be a real commit');
});

Then('no worktree {string} should exist in workspace {string}', function (this: SapphireWorld, worktreeName: string, wsId: string) {
  const worktree = path.join(profileDir(this), 'workspaces', wsId, worktreeName);
  assert.ok(!fs.existsSync(worktree), `worktree should be gone: ${worktree}`);
});

// --- Clone Repo -----------------------------------------------------------

When('the user clicks {string} in the New Task form', async function (this: SapphireWorld, _label: string) {
  await this.bridge.newTask({ action: 'cloneRepo' });
  await this.bridge.waitFor(
    'the Clone Repo dialog with its URL focused',
    () => this.bridge.state(),
    (s) => s.cloneRepo?.focus === 'url' && s.editingText,
  );
});

When('the user clones the repo {string}', async function (this: SapphireWorld, repoName: string) {
  const url = `file://${bareRepoPath(this, repoName)}`;
  await this.bridge.cloneRepo({ action: 'setUrl', value: url });
  const dialog = await waitForCloneRepo(this, 'the URL to be filled in', (d) => d.url === url);
  assert.strictEqual(dialog.hint, `Clones to:  ${path.join(profileDir(this), 'repos', repoName)}`);
  await this.bridge.cloneRepo({ action: 'submit' });
  // The dialog streams progress, then closes itself once the clone (detached) is done.
  await this.bridge.waitFor('the Clone Repo dialog to close', () => this.bridge.state(), (s) => s.cloneRepo === null, 30_000);
});

Then('the repo {string} should exist detached in profile {string}', function (this: SapphireWorld, repoName: string, profileName: string) {
  const dir = path.join(this.fixtureRoot!, profileName, 'repos', repoName);
  assert.ok(fs.existsSync(path.join(dir, '.git')), `clone should exist: ${dir}`);
  assert.ok(fs.existsSync(path.join(dir, 'file.txt')), `clone should be checked out: ${dir}`);
  assert.strictEqual(git('rev-parse --abbrev-ref HEAD', dir).trim(), 'HEAD', 'cloned repo HEAD should be detached');
});

Then('the New Task form repo list should include {string}', async function (this: SapphireWorld, repoName: string) {
  await waitForNewTask(this, `${repoName} in the New Task dialog's list`, (dialog) => dialog.repos.some((r) => r.name === repoName));
});

Then('the New Task form should have repo {string} selected', async function (this: SapphireWorld, repoName: string) {
  await waitForNewTask(this, `${repoName} to be selected`, (dialog) => dialog.repos.some((r) => r.name === repoName && r.selected));
});
