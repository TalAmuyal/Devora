import { After, AfterStep, Before, BeforeAll, Status } from '@cucumber/cucumber';
import { spawn, type ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { cleanupFixtures, writeTestConfig } from '../../../project-ember/tests/support/fixture-helper';
import { Bridge } from './bridge';
import { SapphireWorld } from './world';

const PROJECT_DIR = path.resolve(import.meta.dirname, '../..');
const APP_BINARY = path.join(PROJECT_DIR, process.env.SAPPHIRE_APP!, 'Contents/MacOS/Devora Sapphire');
const FAILURE_CAPTURE_DIR = path.join(PROJECT_DIR, 'test-results/failures');

/** The app's stderr, per process, for failure captures. */
const stderrOf = new WeakMap<ChildProcess, string[]>();

BeforeAll(function () {
  if (!fs.existsSync(APP_BINARY)) {
    throw new Error(`${APP_BINARY} is missing: run \`mise run build\` (\`mise run test-acceptance\` does)`);
  }
  fs.rmSync(FAILURE_CAPTURE_DIR, { recursive: true, force: true });
});

/** Every scenario gets a fresh app, on a config of its own: nothing a scenario does can leak into the next. */
Before(async function (this: SapphireWorld) {
  this.testConfigPath = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'sapphire-config-')), 'config.json');
  writeTestConfig(this.testConfigPath, []);
  const { app, port } = await launch(this.testConfigPath);
  this.app = app;
  this.bridge = new Bridge(port);
  // The bridge starts once the launch is done, but the startup tab reaches the store's state on a later frame; steps that read the state before acting need it settled.
  await this.bridge.waitFor(
    'the startup shell tab to run',
    () => this.bridge.state(),
    (s) => s.tabs.length > 0 && s.tabs.every((tab) => tab.terminal === 'running'),
  );
});

AfterStep(async function (this: SapphireWorld, { result, pickle, pickleStep }) {
  if (result.status !== Status.FAILED) return;
  await captureFailure(this, `${pickle.name}--${pickleStep.text}`);
});

After(async function (this: SapphireWorld) {
  if (this.app) await quit(this.app);
  fs.rmSync(path.dirname(this.testConfigPath), { recursive: true, force: true });
  if (this.fixtureRoot) cleanupFixtures(this.fixtureRoot);
});

async function launch(configPath: string): Promise<{ app: ChildProcess; port: number }> {
  // Only what a login session would provide, so the developer's environment (and a Devora app around the test run) doesn't leak into the app's terminals.
  const env: Record<string, string> = {};
  for (const key of ['HOME', 'SHELL', 'TMPDIR', 'USER', 'LANG']) {
    if (process.env[key]) env[key] = process.env[key]!;
  }
  env.PATH = (process.env.PATH ?? '')
    .split(':')
    .filter((p) => !p.includes('.app/Contents') && !p.includes('/Devora/'))
    .join(':');

  // Launched directly rather than through `open`, so the harness owns the process and its stderr; Sapphire takes no bare arguments, so a window opens.
  const app = spawn(APP_BINARY, [], {
    env: { ...env, DEVORA_TEST_MODE: '1', DEVORA_CONFIG_PATH: configPath },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const stderr: string[] = [];
  stderrOf.set(app, stderr);

  const port = await new Promise<number>((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error('Timed out waiting for the test bridge port on stderr')), 30_000);
    app.stderr!.on('data', (chunk: Buffer) => {
      const text = chunk.toString();
      stderr.push(text);
      const match = text.match(/Devora test harness on port (\d+)/);
      if (match) {
        clearTimeout(timeout);
        resolve(parseInt(match[1], 10));
      }
    });
    app.on('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    app.on('exit', (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`The app exited (${code ?? signal}) before its test bridge started:\n${stderr.join('')}`));
    });
  });
  return { app, port };
}

async function quit(app: ChildProcess): Promise<void> {
  if (app.exitCode !== null || app.signalCode !== null) return;
  const exited = new Promise((resolve) => app.once('exit', resolve));
  app.kill('SIGTERM');
  await exited;
}

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
  fs.mkdirSync(FAILURE_CAPTURE_DIR, { recursive: true });
  const file = path.join(FAILURE_CAPTURE_DIR, `${label.replace(/[^a-zA-Z0-9_-]+/g, '_').slice(0, 150)}.txt`);
  fs.writeFileSync(file, sections.join('\n\n') + '\n');
}
