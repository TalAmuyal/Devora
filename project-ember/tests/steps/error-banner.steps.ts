import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';

const TEST_REPO_NAME = 'test-repo';
const BANNER_SELECTOR = '.error-banner-stack .ws-error-notification';

When(
  'the worktree repo of workspace {string} becomes corrupted',
  function (this: EmberWorld, wsId: string) {
    // Removing the worktree's .git link makes every git operation in it fail, simulating out-of-band breakage after the hub cached a healthy status.
    const gitLink = path.join(
      this.fixtureRoot!, 'Work', 'workspaces', wsId, TEST_REPO_NAME, '.git',
    );
    assert.ok(fs.existsSync(gitLink), `expected ${gitLink} to exist before corrupting it`);
    fs.rmSync(gitLink, { recursive: true, force: true });
  },
);

Given(
  'the task.json of workspace {string} is malformed',
  function (this: EmberWorld, wsId: string) {
    const taskPath = path.join(this.fixtureRoot!, 'Work', 'workspaces', wsId, 'task.json');
    assert.ok(fs.existsSync(taskPath), `expected ${taskPath} to exist before corrupting it`);
    fs.writeFileSync(taskPath, '{not json');
  },
);

Then(
  'an error banner containing {string} should be visible',
  async function (this: EmberWorld, text: string) {
    await this.driver.pollFor(
      `return [...document.querySelectorAll('${BANNER_SELECTOR}')]
        .some((el) => el.textContent.includes(${JSON.stringify(text)}))`,
      true,
      5_000,
    );
  },
);

When(
  'the user dismisses the error banner',
  async function (this: EmberWorld) {
    const ui = new UIDriver(this.driver);
    await ui.click(`${BANNER_SELECTOR} .ws-error-notification-dismiss`);
  },
);

Then(
  'no error banners should be visible',
  async function (this: EmberWorld) {
    await this.driver.pollFor(
      `return document.querySelectorAll('${BANNER_SELECTOR}').length`,
      0,
      3_000,
    );
  },
);

Then(
  'the recorded errors should include {string}',
  async function (this: EmberWorld, text: string) {
    // Scraping drains the list, which also keeps the After-hook's no-unexpected-errors check green for intentional-error scenarios.
    const errors: string[] = await this.driver.eval('return window.__scrapeErrors()');
    assert.ok(
      errors.some((e) => e.includes(text)),
      `Expected a recorded error containing "${text}", got: ${JSON.stringify(errors)}`,
    );
  },
);
