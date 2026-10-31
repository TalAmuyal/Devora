import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import type { AppState } from '../support/bridge';
import { SapphireWorld } from '../support/world';

// The banner phrases are Ember's (`project-ember/tests/steps/error-banner.steps.ts`).

Given("the workspaces directory of the profile can't be read", function (this: SapphireWorld) {
  // A file in the directory's place: listing it fails, where a missing one would just list nothing.
  const workspaces = path.join(this.profilePath!, 'workspaces');
  fs.rmSync(workspaces, { recursive: true });
  fs.writeFileSync(workspaces, '');
});

Given('the task.json of workspace {string} is malformed', function (this: SapphireWorld, id: string) {
  const taskPath = path.join(this.profilePath!, 'workspaces', id, 'task.json');
  assert.ok(fs.existsSync(taskPath), `expected ${taskPath} to exist before corrupting it`);
  fs.writeFileSync(taskPath, '{not json');
});

function bannerContaining(state: AppState, text: string) {
  return state.errorBanners.find((banner) => banner.message.includes(text));
}

Then('an error banner containing {string} should be visible', async function (this: SapphireWorld, text: string) {
  await this.bridge.waitFor(`an error banner containing "${text}"`, () => this.bridge.state(), (s) => !!bannerContaining(s, text));
});

Then(
  'the error banner containing {string} should count {int}',
  async function (this: SapphireWorld, text: string, count: number) {
    await this.bridge.waitFor(
      `the error banner containing "${text}" to count ${count}`,
      () => this.bridge.state(),
      (s) => bannerContaining(s, text)?.count === count,
    );
  },
);

When('the user dismisses the error banner', async function (this: SapphireWorld) {
  // The click itself is the XCUITests' (`ErrorBannerUITests`); this is the command its × dispatches.
  const banners = (await this.bridge.state()).errorBanners;
  assert.strictEqual(banners.length, 1, `expected one banner, got ${JSON.stringify(banners)}`);
  await this.bridge.dispatch({ command: 'dismissError', id: banners[0].id });
});

Then('no error banners should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('no error banners', () => this.bridge.state(), (s) => s.errorBanners.length === 0);
});

Then('the recorded errors should include {string}', async function (this: SapphireWorld, text: string) {
  // Scraping drains the list, which also keeps the After hook's no-unexpected-errors check green for intentional-error scenarios.
  const errors = await this.bridge.scrapeErrors();
  assert.ok(
    errors.some((e) => e.includes(text)),
    `Expected a recorded error containing "${text}", got: ${JSON.stringify(errors)}`,
  );
});

Then('the log file should contain {string}', async function (this: SapphireWorld, text: string) {
  const { logPath } = await this.bridge.state();
  assert.ok(path.basename(logPath).startsWith('devora-sapphire-'), logPath);
  const contents = fs.readFileSync(logPath, 'utf8');
  assert.ok(contents.includes(text), `Expected ${logPath} to contain "${text}", got:\n${contents}`);
});
