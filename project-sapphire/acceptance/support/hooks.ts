import { writeTestConfig } from './test-config';
import { After, AfterStep, Before, BeforeAll, Status } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { cleanupFixtures } from '../../../project-ember/tests/support/fixture-helper';
import { APP_BINARY, BUNDLED_APP_BINARY, hasExited, shutdown, waitUntilReady } from './app';
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
Before(async function (this: SapphireWorld, { pickle }) {
  this.scenarioDir = fs.mkdtempSync(path.join(os.tmpdir(), 'sapphire-scenario-'));
  this.testConfigPath = path.join(this.scenarioDir, 'config.json');
  this.stateDir = path.join(this.scenarioDir, 'state');
  writeTestConfig(this.testConfigPath, []);
  this.bundledTools = pickle.tags.some(tag => tag.name === '@bundled-tools');
  this.nativeTranscript = process.env.SAPPHIRE_NATIVE_TRANSCRIPT === '1';
  if (this.bundledTools) {
    // No injected PATH: devora-terminal must discover the real bundle Resources.
    const resources = path.resolve(path.dirname(BUNDLED_APP_BINARY), '../Resources');
    for (const tool of ['bundled-apps/debi', 'bundled-apps/debi-go', 'git-shortcuts/gcl']) {
      fs.accessSync(path.join(resources, tool), fs.constants.X_OK);
    }
    this.home = path.join(this.scenarioDir, 'home');
    fs.mkdirSync(this.home);
  }
  // As in Ember, no fixture file selects the real core runner for @real-health.
  if (!pickle.tags.some(tag => tag.name === '@real-health')) {
    fs.copyFileSync(path.resolve(import.meta.dirname, '../../../acceptance/fixtures/health-report.json'), path.join(this.scenarioDir, 'health-report.json'));
  }
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
      const apps = [{ app: this.app, bridge: this.bridge }, this.second].filter(app => app?.app);
      const stopped = await Promise.allSettled(apps.map(app => shutdown(app!)));
      const failure = stopped.find(result => result.status === 'rejected');
      if (failure?.status === 'rejected') throw failure.reason;
      fs.rmSync(this.scenarioDir, { recursive: true, force: true });
      if (this.fixtureRoot) cleanupFixtures(this.fixtureRoot);
    }
  }
});
