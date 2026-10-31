import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { createTestFixtureRoot, createTestProfile, writeTestConfig } from '../support/fixture-helper';
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
} from '../support/task-creation-fixture';

const PROFILE = 'Work';

function profileDir(world: EmberWorld): string {
  return path.join(world.fixtureRoot!, PROFILE);
}

// --- Fixtures -------------------------------------------------------------

Given(
  'an origin-backed profile {string} with repo {string}',
  function (this: EmberWorld, profileName: string, repoName: string) {
    this.fixtureRoot = createTestFixtureRoot();
    const profilePath = createTestProfile(this.fixtureRoot, profileName);
    createOriginBackedRepo(this.fixtureRoot, profilePath, repoName);
    writeTestConfig(this.testConfigPath!, [profilePath]);
  },
);

Given(
  'an additional origin-backed repo {string}',
  function (this: EmberWorld, repoName: string) {
    createOriginBackedRepo(this.fixtureRoot!, profileDir(this), repoName);
  },
);

Given('the prepare-command writes a marker', function (this: EmberWorld) {
  setPrepareCommand(this.testConfigPath!, `touch ${PREPARE_MARKER}`);
});

Given('the prepare-command sleeps', function (this: EmberWorld) {
  setPrepareCommand(this.testConfigPath!, 'sleep 30');
});

Given(
  'an inactive workspace {string} with a worktree of {string}',
  function (this: EmberWorld, wsId: string, repoName: string) {
    createInactiveWorkspaceWithWorktree(profileDir(this), wsId, repoName);
  },
);

Given(
  'a cached file {string} in workspace {string} repo {string}',
  function (this: EmberWorld, file: string, wsId: string, repoName: string) {
    createCachedFile(profileDir(this), wsId, repoName, file);
  },
);

Given(
  'the origin of repo {string} has advanced',
  function (this: EmberWorld, repoName: string) {
    this.expectedHead = advanceOrigin(profileDir(this), repoName);
  },
);

// --- Actions --------------------------------------------------------------

When(
  'the user creates a task {string} selecting repo {string}',
  async function (this: EmberWorld, title: string, repoName: string) {
    this.sessionsBeforeTask = await this.driver.eval('return window.__test.sessionManager.getSessionCount()');
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.ws-new-btn');
    await ui.click('.ws-new-btn');
    await this.driver.pollFor(`return document.querySelector('.new-task-dialog') !== null`, true, 5_000);

    await ui.typeIntoInput('.ws-new-form-input', title);

    await this.driver.pollFor(
      `return Array.from(document.querySelectorAll('.repo-list-item')).some((i) => (i.textContent || '').trim() === ${JSON.stringify(repoName)})`,
      true,
      5_000,
    );
    await this.driver.eval(`
      const item = Array.from(document.querySelectorAll('.repo-list-item'))
        .find((i) => (i.textContent || '').trim() === ${JSON.stringify(repoName)});
      need(item, 'repo checkbox ' + ${JSON.stringify(repoName)}).querySelector('input[type=checkbox]').checked = true;
    `);

    await ui.click('.ws-new-form-create');
  },
);

async function waitForPreparing(world: EmberWorld): Promise<void> {
  await world.driver.pollFor(
    `return Array.from(document.querySelectorAll('.task-creation-step-label')).some((s) => (s.textContent || '').includes('Preparing'))`,
    true,
    20_000,
  );
}

Given('the task creation is preparing', async function (this: EmberWorld) {
  await waitForPreparing(this);
});

When(
  'the user cancels the task creation once it is preparing',
  async function (this: EmberWorld) {
    await waitForPreparing(this);
    const ui = new UIDriver(this.driver);
    await ui.click('.task-creation-action');
  },
);

// --- Assertions -----------------------------------------------------------

Then('a task-creation progress overlay should be visible', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.waitForElement('.task-creation', 5_000);
});

Then('the task creation should complete', async function (this: EmberWorld) {
  await this.driver.pollFor(`return document.querySelector('.task-creation') === null`, true, 30_000);
});

Then('the active session should be connected', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `return window.__test.sessionManager.getActiveSession()?.getPtyId() != null`,
    true,
    30_000,
  );
});

Then(
  'workspace {string} should be active with title {string}',
  async function (this: EmberWorld, wsId: string, title: string) {
    await assertWorkspaceActiveWithTitle(profileDir(this), wsId, title);
  },
);

Then('no workspace {string} should exist', async function (this: EmberWorld, wsId: string) {
  await assertNoWorkspace(profileDir(this), wsId);
});

Then(
  'the worktree of {string} in workspace {string} should be at the latest origin commit',
  function (this: EmberWorld, repoName: string, wsId: string) {
    assertWorktreeAt(profileDir(this), wsId, repoName, this.expectedHead);
  },
);

Then(
  'the cached file {string} should exist in workspace {string} repo {string}',
  function (this: EmberWorld, file: string, wsId: string, repoName: string) {
    assertCachedFile(profileDir(this), wsId, repoName, file);
  },
);

Then(
  'the prepare marker should exist in workspace {string} repo {string}',
  async function (this: EmberWorld, wsId: string, repoName: string) {
    await assertPrepareMarker(profileDir(this), wsId, repoName);
  },
);

Then('the task-creation tab should close', async function (this: EmberWorld) {
  await this.driver.pollFor(`return document.querySelector('.task-creation') === null`, true, 30_000);
  await this.driver.pollFor(`return window.__test.sessionManager.getSessionCount()`, this.sessionsBeforeTask ?? 0, 30_000);
});

Then('workspace {string} should be inactive', async function (this: EmberWorld, wsId: string) {
  await assertWorkspaceInactive(profileDir(this), wsId);
});
