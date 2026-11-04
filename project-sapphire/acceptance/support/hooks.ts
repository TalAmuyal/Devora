import { After, AfterStep, Before, BeforeAll, Status } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { cleanupFixtures, writeTestConfig } from '../../../project-ember/tests/support/fixture-helper';
import { APP_BINARY, hasExited, stop, waitUntilReady } from './app';
import { SapphireWorld } from './world';
import { stopTestClaude } from './claude';
import { captureFailure, FAILURE_CAPTURE_DIR } from './failure-capture';

BeforeAll(function () {
  if (!fs.existsSync(APP_BINARY)) {
    throw new Error(`${APP_BINARY} is missing: run \`mise run build\` (\`mise run test-acceptance\` does)`);
  }
  fs.rmSync(FAILURE_CAPTURE_DIR, { recursive: true, force: true });
});

/** Every scenario gets a fresh app, on a config and an app state of its own: nothing a scenario does can leak into the next. */
Before(async function (this: SapphireWorld) {
  this.scenarioDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sapphire-scenario-'));
  this.testConfigPath = path.join(this.scenarioDir, 'config.json');
  this.stateDir = path.join(this.scenarioDir, 'state');
  writeTestConfig(this.testConfigPath, []);
  fs.copyFileSync(path.resolve(import.meta.dirname, '../../../acceptance/fixtures/health-report.json'), path.join(this.scenarioDir, 'health-report.json'));
  await this.launch();
  // A fresh state restores no tabs, so the launch opens the Workspace Hub.
  await waitUntilReady(this.bridge);
});

AfterStep(async function (this: SapphireWorld, { result, pickle, pickleStep }) {
  if (result.status !== Status.FAILED) return;
  await captureFailure(this, `${pickle.name}--${pickleStep.text}`);
});

/** As Ember's: an error no step expected fails the scenario; steps that expect one scrape it ("the recorded errors should include"). */
After(async function (this: SapphireWorld) {
  try {
    const errors = this.app && !hasExited(this.app) ? await this.bridge.scrapeErrors() : [];
    if (errors.length > 0) {
      throw new Error(`Errors shown during the scenario:\n${errors.map((e) => `  - ${e}`).join('\n')}`);
    }
  } finally {
    try { await stopTestClaude(this); } finally {
      for (const app of [this.app, this.second?.app]) {
        if (app) await stop(app, 'SIGTERM');
      }
      fs.rmSync(this.scenarioDir, { recursive: true, force: true });
      if (this.fixtureRoot) cleanupFixtures(this.fixtureRoot);
    }
  }
});
