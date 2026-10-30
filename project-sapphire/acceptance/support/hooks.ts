import { After, AfterStep, Before, BeforeAll, Status } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { cleanupFixtures, writeTestConfig } from '../../../project-ember/tests/support/fixture-helper';
import { APP_BINARY, stderrOf, stop, waitUntilReady } from './app';
import { SapphireWorld } from './world';

const PROJECT_DIR = path.resolve(import.meta.dirname, '../..');
const FAILURE_CAPTURE_DIR = path.join(PROJECT_DIR, 'test-results/failures');

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
  await this.launch();
  // A fresh state restores no tabs, so the launch opens one shell tab.
  await waitUntilReady(this.bridge);
});

AfterStep(async function (this: SapphireWorld, { result, pickle, pickleStep }) {
  if (result.status !== Status.FAILED) return;
  await captureFailure(this, `${pickle.name}--${pickleStep.text}`);
});

After(async function (this: SapphireWorld) {
  for (const app of [this.app, this.second?.app]) {
    if (app) await stop(app, 'SIGTERM');
  }
  fs.rmSync(this.scenarioDir, { recursive: true, force: true });
  if (this.fixtureRoot) cleanupFixtures(this.fixtureRoot);
});

/** Saves the app's state, the active terminal's text and the app's stderr to `test-results/failures/`; in CI, they are uploaded with the failed run. */
async function captureFailure(world: SapphireWorld, label: string): Promise<void> {
  const sections: string[] = [];
  try {
    sections.push(`## State\n${JSON.stringify(await world.bridge.state(), null, 2)}`);
    sections.push(`## Active terminal\n${await world.bridge.terminalText()}`);
  } catch (error) {
    sections.push(`## Bridge error\n${error}`);
  }
  sections.push(`## stderr\n${(stderrOf.get(world.app) ?? []).join('')}`);
  if (world.second) sections.push(`## Second app's stderr\n${(stderrOf.get(world.second.app) ?? []).join('')}`);
  fs.mkdirSync(FAILURE_CAPTURE_DIR, { recursive: true });
  const file = path.join(FAILURE_CAPTURE_DIR, `${label.replace(/[^a-zA-Z0-9_-]+/g, '_').slice(0, 150)}.txt`);
  fs.writeFileSync(file, sections.join('\n\n') + '\n');
}
