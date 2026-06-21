import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { GIT_TEST_IDENTITY } from '../support/git-test-env';

const GIT_ENV = { ...process.env, ...GIT_TEST_IDENTITY };

function git(args: string, cwd: string): string {
  return execSync(`git ${args}`, { cwd, env: GIT_ENV, stdio: ['ignore', 'pipe', 'ignore'] }).toString();
}

function bareRepoPath(world: EmberWorld, repoName: string): string {
  return path.join(world.fixtureRoot!, `${repoName}.git`);
}

function clonedRepoDir(world: EmberWorld, profileName: string, repoName: string): string {
  return path.join(world.fixtureRoot!, profileName, 'repos', repoName);
}

// A standalone bare repo with one commit on main, NOT placed under any profile's repos/ — the user clones it via its file:// URL.
// (Distinct from createOriginBackedRepo, which pre-populates repos/.)
Given('a bare repo {string} available to clone', function (this: EmberWorld, repoName: string) {
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

When('the user opens the New Task form', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.click('.ws-new-btn');
  await ui.waitForElement('.ws-new-form', 5_000);
});

When(
  'the user clicks {string} in the New Task form',
  async function (this: EmberWorld, _label: string) {
    const ui = new UIDriver(this.driver);
    await ui.click('.ws-new-form-clone');
    await ui.waitForElement('.clone-repo-dialog', 5_000);
  },
);

When('the user clones the repo {string}', async function (this: EmberWorld, repoName: string) {
  const ui = new UIDriver(this.driver);
  await ui.waitForElement('.clone-repo-dialog', 5_000);
  await ui.typeIntoInput('.clone-repo-dialog-input', `file://${bareRepoPath(this, repoName)}`);
  await ui.click('.clone-repo-dialog-clone');
  // The dialog streams progress, then closes itself once the clone (detached) is done.
  await this.driver.pollFor(
    `return document.querySelector('.clone-repo-dialog') === null`,
    true,
    30_000,
  );
});

Then(
  'the repo {string} should exist detached in profile {string}',
  function (this: EmberWorld, repoName: string, profileName: string) {
    const dir = clonedRepoDir(this, profileName, repoName);
    assert.ok(fs.existsSync(path.join(dir, '.git')), `clone should exist: ${dir}`);
    assert.ok(fs.existsSync(path.join(dir, 'file.txt')), `clone should be checked out: ${dir}`);
    const branch = git('rev-parse --abbrev-ref HEAD', dir).trim();
    assert.strictEqual(branch, 'HEAD', 'cloned repo HEAD should be detached');
  },
);

Then(
  'the New Task form repo list should include {string}',
  async function (this: EmberWorld, repoName: string) {
    await this.driver.pollFor(
      `return Array.from(document.querySelectorAll('.ws-new-form .repo-list-item'))
        .some((i) => (i.textContent || '').trim() === ${JSON.stringify(repoName)})`,
      true,
      5_000,
    );
  },
);
