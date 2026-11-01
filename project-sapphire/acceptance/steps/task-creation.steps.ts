import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { createTestRepo } from '../../../project-ember/tests/support/fixture-helper';
import {
  PREPARE_MARKER,
  advanceOrigin,
  assertCachedFile,
  assertNoWorkspace,
  assertPrepareMarker,
  assertWorkspaceActiveWithTitle,
  assertWorkspaceInactive,
  assertWorktreeAt,
  createCachedFile,
  createInactiveWorkspaceWithWorktree,
  createOriginBackedRepo,
  setPrepareCommand,
} from '../../../project-ember/tests/support/task-creation-fixture';
import type { AppState, NewTaskState, TabState } from '../support/bridge';
import { SapphireWorld } from '../support/world';
import { addProfile } from './workspace.steps';
import { typeText } from './workspace-hub.steps';

// The phrases are Ember's (`project-ember/tests/steps/task-creation.steps.ts` and the New Task steps of its `workspace-hub.steps.ts`); the dialog is typed into with keys and clicked through the bridge's `new-task` route.

function profileDir(world: SapphireWorld): string {
  assert.ok(world.profilePath, 'no profile was made');
  return world.profilePath;
}

async function waitForNewTask(world: SapphireWorld, what: string, done: (dialog: NewTaskState) => boolean): Promise<NewTaskState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.newTask !== null && done(s.newTask));
  return state.newTask!;
}

/** Clicks "+ New Task" and waits for the dialog, whose title field takes the keyboard. */
async function openNewTaskForm(world: SapphireWorld): Promise<void> {
  await world.bridge.hub({ action: 'newTask' });
  await world.bridge.waitFor(
    'the New Task dialog with its title focused',
    () => world.bridge.state(),
    (s) => s.newTask?.focus === 'title' && s.editingText,
  );
}

function tabWithProgress(state: AppState): TabState | undefined {
  return state.tabs.find((tab) => tab.progress !== null);
}

function activeTab(state: AppState): TabState | undefined {
  return state.tabs.find((tab) => tab.id === state.activeTab);
}

// --- Fixtures -------------------------------------------------------------

Given('a profile {string} with repos {string}', function (this: SapphireWorld, name: string, repoList: string) {
  addProfile(this, name, (profilePath) => {
    for (const repoName of repoList.split(',').map((r) => r.trim()).filter(Boolean)) createTestRepo(profilePath, repoName);
  });
});

Given('an origin-backed profile {string} with repo {string}', function (this: SapphireWorld, name: string, repoName: string) {
  addProfile(this, name, (profilePath) => createOriginBackedRepo(this.fixtureRoot!, profilePath, repoName));
});

Given('the prepare-command writes a marker', function (this: SapphireWorld) {
  setPrepareCommand(this.testConfigPath, `touch ${PREPARE_MARKER}`);
});

Given('the prepare-command sleeps', function (this: SapphireWorld) {
  setPrepareCommand(this.testConfigPath, 'sleep 30');
});

Given('an inactive workspace {string} with a worktree of {string}', function (this: SapphireWorld, wsId: string, repoName: string) {
  createInactiveWorkspaceWithWorktree(profileDir(this), wsId, repoName);
});

Given('a cached file {string} in workspace {string} repo {string}', function (this: SapphireWorld, file: string, wsId: string, repoName: string) {
  createCachedFile(profileDir(this), wsId, repoName, file);
});

Given('the origin of repo {string} has advanced', function (this: SapphireWorld, repoName: string) {
  this.expectedHead = advanceOrigin(profileDir(this), repoName);
});

Given('the origin of repo {string} is gone', function (this: SapphireWorld, repoName: string) {
  fs.rmSync(path.join(this.fixtureRoot!, `${repoName}.git`), { recursive: true, force: true });
});

// --- The New Task dialog --------------------------------------------------

When('the user opens the New Task form', async function (this: SapphireWorld) {
  await openNewTaskForm(this);
});

Given('the New Task form is open', async function (this: SapphireWorld) {
  await openNewTaskForm(this);
});

Then('the New Task form should show {int} repo(s)', async function (this: SapphireWorld, count: number) {
  await waitForNewTask(this, `${count} repos in the New Task dialog`, (dialog) => dialog.repos.length === count);
});

When('the user filters the New Task form repo list by {string}', async function (this: SapphireWorld, text: string) {
  // The filter comes after the title and "+ Clone Repo" in the dialog's Tab order.
  await this.bridge.press('Tab');
  await this.bridge.press('Tab');
  await this.bridge.waitFor('the filter to have the focus', () => this.bridge.state(), (s) => s.newTask?.focus === 'filter' && s.editingText);
  await typeText(this, text);
  await waitForNewTask(this, `the filter to read "${text}"`, (dialog) => dialog.filter === text);
});

When('the user types {string} into the focused task title input', async function (this: SapphireWorld, key: string) {
  this.lastKeyConsumed = await this.bridge.press(key);
  await waitForNewTask(this, `the title to read "${key}"`, (dialog) => dialog.title === key);
});

Then('the New Task form should still be visible', async function (this: SapphireWorld) {
  const state = await this.bridge.state();
  assert.ok(state.newTask, `the New Task dialog closed; window layers: ${JSON.stringify(state.windowLayers)}`);
});

Then('the keypress should not have been intercepted', function (this: SapphireWorld) {
  assert.strictEqual(this.lastKeyConsumed, false);
});

When('the user duplicates the focused workspace', async function (this: SapphireWorld) {
  await this.bridge.hub({ action: 'duplicate' });
});

Then(
  'the New Task form should be titled {string} with {string} and repo {string} selected',
  async function (this: SapphireWorld, heading: string, title: string, repoName: string) {
    const dialog = await waitForNewTask(this, 'the New Task dialog', () => true);
    assert.deepStrictEqual(
      { heading: dialog.heading, title: dialog.title, selected: dialog.repos.filter((r) => r.selected).map((r) => r.name) },
      { heading, title, selected: [repoName] },
    );
  },
);

When('the user creates the task in the New Task form', async function (this: SapphireWorld) {
  this.sessionsBeforeTask = (await this.bridge.state()).tabs.length;
  await this.bridge.newTask({ action: 'create' });
});

When('the user creates a task {string} selecting repo {string}', async function (this: SapphireWorld, title: string, repoName: string) {
  this.sessionsBeforeTask = (await this.bridge.state()).tabs.length;
  await openNewTaskForm(this);
  await typeText(this, title);
  await waitForNewTask(this, `the title to read "${title}"`, (dialog) => dialog.title === title);
  await this.bridge.newTask({ action: 'toggleRepo', name: repoName });
  await waitForNewTask(this, `${repoName} to be selected`, (dialog) => dialog.repos.some((r) => r.name === repoName && r.selected));
  await this.bridge.newTask({ action: 'create' });
});

// --- Progress -------------------------------------------------------------

async function waitForPreparing(world: SapphireWorld): Promise<TabState> {
  const state = await world.bridge.waitFor(
    'a creation to be preparing',
    () => world.bridge.state(),
    (s) => tabWithProgress(s)?.progress?.steps.some((step) => step.label.startsWith('Preparing')) ?? false,
    20_000,
  );
  return tabWithProgress(state)!;
}

Given('the task creation is preparing', async function (this: SapphireWorld) {
  await waitForPreparing(this);
});

When('the user cancels the task creation once it is preparing', async function (this: SapphireWorld) {
  const tab = await waitForPreparing(this);
  await this.bridge.progressAction(tab.id);
});

Then('a task-creation progress overlay should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the active tab to show its progress page', () => this.bridge.state(), (s) => activeTab(s)?.progress != null, 5_000);
});

Then('the task creation should complete', async function (this: SapphireWorld) {
  await this.bridge.waitFor(
    'the progress page to go',
    () => this.bridge.state(),
    (s) => s.tabs.length > (this.sessionsBeforeTask ?? 0) && tabWithProgress(s) === undefined,
    30_000,
  );
});

Then('the task creation should fail with {string}', async function (this: SapphireWorld, message: string) {
  const state = await this.bridge.waitFor(
    'the creation to fail',
    () => this.bridge.state(),
    (s) => activeTab(s)?.progress?.failure != null,
    30_000,
  );
  const tab = activeTab(state)!;
  assert.strictEqual(tab.kind, 'creationFailed');
  assert.ok(tab.progress!.failure!.includes(message), `the page shows: ${tab.progress!.failure}`);
  assert.strictEqual(tab.progress!.action, 'Close');
  assert.ok(tab.progress!.steps.some((step) => step.state === 'failed'), JSON.stringify(tab.progress!.steps));
});

Then('the active session should be connected', async function (this: SapphireWorld) {
  await this.bridge.waitFor(
    "the active tab's terminal to run in its workspace",
    () => this.bridge.state(),
    (s) => {
      const tab = activeTab(s);
      return tab?.kind === 'workspace' && tab.terminal === 'running' && tab.layers.join() === 'terminal';
    },
    30_000,
  );
});

Then('the task-creation tab should close', async function (this: SapphireWorld) {
  await this.bridge.waitFor(
    'the task-creation tab to close',
    () => this.bridge.state(),
    (s) => tabWithProgress(s) === undefined && s.tabs.length === (this.sessionsBeforeTask ?? 0),
    30_000,
  );
});

// --- The workspace on disk ------------------------------------------------

Then('workspace {string} should be active with title {string}', async function (this: SapphireWorld, wsId: string, title: string) {
  await assertWorkspaceActiveWithTitle(profileDir(this), wsId, title);
});

Then('no workspace {string} should exist', async function (this: SapphireWorld, wsId: string) {
  await assertNoWorkspace(profileDir(this), wsId);
});

Then(
  'the worktree of {string} in workspace {string} should be at the latest origin commit',
  function (this: SapphireWorld, repoName: string, wsId: string) {
    assertWorktreeAt(profileDir(this), wsId, repoName, this.expectedHead);
  },
);

Then(
  'the cached file {string} should exist in workspace {string} repo {string}',
  function (this: SapphireWorld, file: string, wsId: string, repoName: string) {
    assertCachedFile(profileDir(this), wsId, repoName, file);
  },
);

Then('the prepare marker should exist in workspace {string} repo {string}', async function (this: SapphireWorld, wsId: string, repoName: string) {
  await assertPrepareMarker(profileDir(this), wsId, repoName);
});

Then('workspace {string} should be inactive', async function (this: SapphireWorld, wsId: string) {
  await assertWorkspaceInactive(profileDir(this), wsId);
});

// --- Per-tab surfaces -----------------------------------------------------

const ORDINALS: Record<string, number> = { first: 0, second: 1, third: 2 };

function ordinal(word: string): number {
  const index = ORDINALS[word];
  if (index === undefined) throw new Error(`Unknown ordinal: ${word}`);
  return index;
}

When(/^the user switches to the (first|second|third) session$/, async function (this: SapphireWorld, word: string) {
  const { tabs } = await this.bridge.state();
  const id = tabs[ordinal(word)].id;
  await this.bridge.dispatch({ command: 'activateTab', id });
  await this.bridge.waitFor(`tab ${id} to be active`, () => this.bridge.state(), (s) => s.activeTab === id);
});

Then('the {word} session should have a panel overlay', async function (this: SapphireWorld, word: string) {
  const index = ordinal(word);
  await this.bridge.waitFor(`the ${word} tab's progress page`, () => this.bridge.state(), (s) => s.tabs[index]?.progress != null);
});

// Only the active tab is shown, so a tab's page is visible exactly while its tab is the active one.
Then('the panel overlay should be visible', async function (this: SapphireWorld) {
  assert.notStrictEqual(activeTab(await this.bridge.state())?.progress ?? null, null, 'the active tab has no progress page');
});

Then('the panel overlay should not be visible', async function (this: SapphireWorld) {
  assert.strictEqual(activeTab(await this.bridge.state())?.progress ?? null, null, 'the active tab has a progress page');
});

// A creating tab has no terminal of its own until its workspace exists, so the terminal that holds the focus is another tab's: where a key would go if routing followed the focus.
Given("the active session's terminal holds focus", async function (this: SapphireWorld) {
  const { tabs } = await this.bridge.state();
  const terminal = tabs.find((tab) => tab.terminal === 'running');
  assert.ok(terminal, 'no tab has a running terminal');
  await this.bridge.focusTerminal(terminal.id);
  await this.bridge.waitFor('the terminal to hold the focus', () => this.bridge.state(), (s) => s.tabs.some((tab) => tab.id === terminal.id && tab.focused));
});

When('the user presses {string} without first taking focus', async function (this: SapphireWorld, key: string) {
  await this.bridge.press(key);
});
