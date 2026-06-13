import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { createTestFixtureRoot, createTestProfile, writeTestConfig } from '../support/fixture-helper';
import { GIT_TEST_IDENTITY } from '../support/git-test-env';

const PROFILE = 'Work';
const PREPARE_MARKER = 'prepared.marker';
const GIT_ENV = { ...process.env, ...GIT_TEST_IDENTITY };

function git(args: string, cwd: string): string {
  return execSync(`git ${args}`, { cwd, env: GIT_ENV, stdio: ['ignore', 'pipe', 'ignore'] }).toString();
}

function profileDir(world: EmberWorld): string {
  return path.join(world.fixtureRoot!, PROFILE);
}

function wsDir(world: EmberWorld, wsId: string): string {
  return path.join(profileDir(world), 'workspaces', wsId);
}

function sourceRepo(world: EmberWorld, repoName: string): string {
  return path.join(profileDir(world), 'repos', repoName);
}

function setPrepareCommand(world: EmberWorld, command: string): void {
  const config = JSON.parse(fs.readFileSync(world.testConfigPath!, 'utf-8'));
  config['prepare-command'] = command;
  fs.writeFileSync(world.testConfigPath!, JSON.stringify(config));
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

// --- Fixtures -------------------------------------------------------------

Given(
  'an origin-backed profile {string} with repo {string}',
  function (this: EmberWorld, profileName: string, repoName: string) {
    this.fixtureRoot = createTestFixtureRoot();
    const profilePath = createTestProfile(this.fixtureRoot, profileName);

    // A bare "origin" with one commit on main, cloned into <profile>/repos/<repo> so the source repo has the origin remote that fresh creation (fetch + worktree add origin/<default>) needs.
    const bare = path.join(this.fixtureRoot, `${repoName}.git`);
    fs.mkdirSync(bare, { recursive: true });
    git('init --bare -b main', bare);

    const source = path.join(profilePath, 'repos', repoName);
    git(`clone ${JSON.stringify(bare)} ${JSON.stringify(source)}`, this.fixtureRoot);
    fs.writeFileSync(path.join(source, 'file.txt'), 'v1\n');
    git('add .', source);
    git('commit -m c1', source);
    git('push origin main', source);

    writeTestConfig(this.testConfigPath!, [profilePath]);
  },
);

Given('the prepare-command writes a marker', function (this: EmberWorld) {
  setPrepareCommand(this, `touch ${PREPARE_MARKER}`);
});

Given('the prepare-command sleeps', function (this: EmberWorld) {
  setPrepareCommand(this, 'sleep 30');
});

Given(
  'an inactive workspace {string} with a worktree of {string}',
  function (this: EmberWorld, wsId: string, repoName: string) {
    const ws = wsDir(this, wsId);
    fs.mkdirSync(ws, { recursive: true });
    fs.writeFileSync(path.join(ws, 'initialized'), ''); // initialized, no task.json => inactive
    const worktree = path.join(ws, repoName);
    git(`worktree add --detach ${JSON.stringify(worktree)} origin/main`, sourceRepo(this, repoName));
  },
);

Given(
  'a cached file {string} in workspace {string} repo {string}',
  function (this: EmberWorld, file: string, wsId: string, repoName: string) {
    // An untracked stand-in for a dependency cache (node_modules/.venv) that must survive refresh.
    fs.writeFileSync(path.join(wsDir(this, wsId), repoName, file), 'cache');
  },
);

Given(
  'the origin of repo {string} has advanced',
  function (this: EmberWorld, repoName: string) {
    const source = sourceRepo(this, repoName);
    fs.writeFileSync(path.join(source, 'file.txt'), 'v2\n');
    git('add .', source);
    git('commit -m c2', source);
    git('push origin main', source);
    this.expectedHead = git('rev-parse HEAD', source).trim();
  },
);

// --- Actions --------------------------------------------------------------

When(
  'the user creates a task {string} selecting repo {string}',
  async function (this: EmberWorld, title: string, repoName: string) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.ws-new-btn');
    await ui.click('.ws-new-btn');
    await this.driver.pollFor(`return document.querySelector('.ws-new-form') !== null`, true, 5_000);

    await ui.typeIntoInput('.ws-new-form-input', title);

    await this.driver.pollFor(
      `return Array.from(document.querySelectorAll('.ws-new-form-repo-item')).some((i) => (i.textContent || '').trim() === ${JSON.stringify(repoName)})`,
      true,
      5_000,
    );
    await this.driver.eval(`
      const item = Array.from(document.querySelectorAll('.ws-new-form-repo-item'))
        .find((i) => (i.textContent || '').trim() === ${JSON.stringify(repoName)});
      if (!item) throw new Error('repo checkbox not found: ' + ${JSON.stringify(repoName)});
      const checkbox = item.querySelector('input[type=checkbox]');
      checkbox.checked = true;
    `);

    await ui.click('.ws-new-form-create');
  },
);

When(
  'the user cancels the task creation once it is preparing',
  async function (this: EmberWorld) {
    await this.driver.pollFor(
      `return Array.from(document.querySelectorAll('.task-creation-step-label')).some((s) => (s.textContent || '').includes('Preparing'))`,
      true,
      20_000,
    );
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
    const taskPath = path.join(wsDir(this, wsId), 'task.json');
    const deadline = Date.now() + 10_000;
    while (Date.now() < deadline) {
      if (fs.existsSync(taskPath)) {
        const task = JSON.parse(fs.readFileSync(taskPath, 'utf-8'));
        if (task.title === title) return;
      }
      await sleep(100);
    }
    assert.fail(`workspace ${wsId} should be active with title "${title}"`);
  },
);

Then('no workspace {string} should exist', async function (this: EmberWorld, wsId: string) {
  const ws = wsDir(this, wsId);
  const deadline = Date.now() + 10_000;
  while (fs.existsSync(ws) && Date.now() < deadline) {
    await sleep(100);
  }
  assert.strictEqual(fs.existsSync(ws), false, `workspace ${wsId} should not exist`);
});

Then(
  'the worktree of {string} in workspace {string} should be at the latest origin commit',
  function (this: EmberWorld, repoName: string, wsId: string) {
    const head = git('rev-parse HEAD', path.join(wsDir(this, wsId), repoName)).trim();
    assert.strictEqual(head, this.expectedHead, 'worktree should be refreshed to the latest origin commit');
  },
);

Then(
  'the cached file {string} should exist in workspace {string} repo {string}',
  function (this: EmberWorld, file: string, wsId: string, repoName: string) {
    const cachePath = path.join(wsDir(this, wsId), repoName, file);
    assert.ok(fs.existsSync(cachePath), `cached file should survive: ${cachePath}`);
  },
);

Then(
  'the prepare marker should exist in workspace {string} repo {string}',
  async function (this: EmberWorld, wsId: string, repoName: string) {
    const markerPath = path.join(wsDir(this, wsId), repoName, PREPARE_MARKER);
    const deadline = Date.now() + 10_000;
    while (!fs.existsSync(markerPath) && Date.now() < deadline) {
      await sleep(100);
    }
    assert.ok(fs.existsSync(markerPath), `prepare marker should exist: ${markerPath}`);
  },
);

Then('the task-creation tab should close', async function (this: EmberWorld) {
  await this.driver.pollFor(`return document.querySelector('.task-creation') === null`, true, 30_000);
  await this.driver.pollFor(`return window.__test.sessionManager.getSessions().length`, 0, 30_000);
});

Then('workspace {string} should be inactive', async function (this: EmberWorld, wsId: string) {
  const ws = wsDir(this, wsId);
  const taskPath = path.join(ws, 'task.json');
  const deadline = Date.now() + 10_000;
  while (fs.existsSync(taskPath) && Date.now() < deadline) {
    await sleep(100);
  }
  assert.ok(fs.existsSync(ws), `workspace ${wsId} directory should still exist`);
  assert.strictEqual(fs.existsSync(taskPath), false, `workspace ${wsId} should be inactive (no task.json)`);
});
