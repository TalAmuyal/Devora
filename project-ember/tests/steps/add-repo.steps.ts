import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { GIT_TEST_IDENTITY } from '../support/git-test-env';

const PROFILE = 'Work';
const GIT_ENV = { ...process.env, ...GIT_TEST_IDENTITY };

function git(args: string, cwd: string): string {
  return execSync(`git ${args}`, { cwd, env: GIT_ENV, stdio: ['ignore', 'pipe', 'ignore'] }).toString();
}

function wsDir(world: EmberWorld, wsId: string): string {
  return path.join(world.fixtureRoot!, PROFILE, 'workspaces', wsId);
}

When(
  'the user adds repo {string} with postfix {string}',
  async function (this: EmberWorld, repoName: string, postfix: string) {
    const ui = new UIDriver(this.driver);
    await ui.waitForElement('.add-repo-dialog', 5_000);

    await this.driver.eval(`
      const item = Array.from(document.querySelectorAll('.add-repo-dialog .repo-list-item'))
        .find((i) => (i.textContent || '').trim() === ${JSON.stringify(repoName)});
      if (!item) throw new Error('repo option not found: ' + ${JSON.stringify(repoName)});
      item.querySelector('input[type=radio]').checked = true;
    `);

    if (postfix) {
      await ui.typeIntoInput('.add-repo-dialog-input', postfix);
    }
    await ui.click('.add-repo-dialog-add');

    // The dialog streams progress, then closes itself once the worktree is ready.
    await this.driver.pollFor(
      `return document.querySelector('.add-repo-dialog') === null`,
      true,
      30_000,
    );
  },
);

Then(
  'the worktree {string} should exist in workspace {string}',
  function (this: EmberWorld, worktreeName: string, wsId: string) {
    const worktree = path.join(wsDir(this, wsId), worktreeName);
    assert.ok(fs.existsSync(path.join(worktree, '.git')), `worktree should exist: ${worktree}`);
    // Checked out from origin/main, so the committed file is present.
    assert.ok(
      fs.existsSync(path.join(worktree, 'file.txt')),
      `worktree should be checked out: ${worktree}`,
    );
    const head = git('rev-parse HEAD', worktree).trim();
    assert.match(head, /^[0-9a-f]{40}$/, 'worktree HEAD should be a real commit');
  },
);
