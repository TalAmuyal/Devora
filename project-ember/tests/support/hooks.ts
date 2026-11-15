import { Before, After, AfterStep, BeforeAll, AfterAll, Status } from '@cucumber/cucumber';
import { spawn, ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import {
  DEFAULT_BUNDLE_DIR,
  assertBundleComplete,
  computeCurrentFingerprint,
  findAppBundle,
  findRawBinary,
  readBundleFingerprint,
} from '../../scripts/app-bundle';
import { AppDriver } from './app-driver';
import { FakeClaudeServer, STRUCTURED_LOG_BASE_DIR } from './fake-claude-server';
import { CLAUDE_CONFIG_PATH, cleanupWorkspace, stopClaudeCode } from './claude-helper';
import { FAILURE_CAPTURE_DIR, canCaptureScreen, captureFailure } from './failure-capture';
import { cleanupFixtures, writeTestConfig } from './fixture-helper';
import { EmberWorld } from './world';

/** How long one launch may take in all, from spawning the app to its test namespace; a normal launch takes 1 to 10 s. */
const LAUNCH_BUDGET_MS = 60_000;
/** The Cucumber timeout of a hook or step that launches the app: above the launch's budget, so a slow launch fails with the launch's own error. */
export const LAUNCH_STEP_TIMEOUT_MS = LAUNCH_BUDGET_MS + 15_000;
/** How many of the app's latest stderr lines a failure keeps. */
const STDERR_LINES = 500;

let appProcess: ChildProcess | null = null;
/** The app's stderr since its last launch, its latest lines only, and the log it named there: a failure's evidence. */
let appStderr: string[] = [];
let appLogPath: string | null = null;
/** A launch that failed: every later scenario fails with it at once, rather than launching again. */
let launchFailure: Error | null = null;
let testHarnessPort: number | null = null;
let ipcPort: number | null = null;
let fakeClaudeServer: FakeClaudeServer | null = null;
let testConfigPath: string | null = null;
let testConfigDir: string | null = null;

/*
 * Resolve the app binary to test, refusing artifacts that would make the run meaningless: a bundle whose BUILD_FINGERPRINT does not match the working tree (tests would silently exercise stale code), an incomplete bundle, or a raw binary (no bundled-apps/, so Claude scenarios fail confusingly).
 * EMBER_E2E_PREBUILT=1 opts out of all checks: test exactly what exists.
 */
function findAppBinary(): string {
  const prebuilt = process.env.EMBER_E2E_PREBUILT === '1';
  const bundle = findAppBundle();

  if (bundle) {
    if (prebuilt) {
      console.warn(
        `EMBER_E2E_PREBUILT=1 -- testing ${bundle.appDir} as-is (freshness and completeness not verified)`,
      );
      return fs.realpathSync(bundle.binaryPath);
    }
    const builtFingerprint = readBundleFingerprint(bundle.resourcesDir);
    const currentFingerprint = computeCurrentFingerprint();
    if (builtFingerprint !== currentFingerprint) {
      throw new Error(
        `App bundle at ${bundle.appDir} is stale: its fingerprint ` +
          `(${builtFingerprint?.slice(0, 12) ?? 'missing'}) does not match the working tree ` +
          `(${currentFingerprint.slice(0, 12)}…). Run \`mise test-e2e\` (auto-rebuilds) ` +
          'or set EMBER_E2E_PREBUILT=1 to test it as-is.',
      );
    }
    assertBundleComplete(bundle.resourcesDir);
    console.log(`Testing ${bundle.appDir} (fingerprint ${currentFingerprint.slice(0, 12)}…)`);
    return fs.realpathSync(bundle.binaryPath);
  }

  const rawBinary = findRawBinary();
  if (!rawBinary) {
    throw new Error(
      `App binary not found: no .app bundle in ${DEFAULT_BUNDLE_DIR} and no raw binary in ` +
        'src-tauri/target/{release,debug}. Run `mise test-e2e` (auto-rebuilds) or ' +
        '`mise run build-ember-app` at the repo root.',
    );
  }
  if (!prebuilt) {
    throw new Error(
      `No app bundle found in ${DEFAULT_BUNDLE_DIR} (only a raw binary at ${rawBinary}). ` +
        'Run `mise test-e2e` (auto-rebuilds), or set EMBER_E2E_PREBUILT=1 to test the raw ' +
        'binary -- bundled-apps/ will be unavailable, so Claude scenarios will fail.',
    );
  }
  console.warn(
    `EMBER_E2E_PREBUILT=1 -- testing raw binary ${rawBinary}; bundled-apps/ is unavailable, ` +
      'so Claude scenarios will fail',
  );
  return rawBinary;
}

async function waitForReady(port: number, timeoutMs: number): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (await bridgeAnswers(port, 1_000)) return;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`App did not become ready within ${timeoutMs}ms`);
}

/** Whether the app's test bridge answers `/test/ready` within `timeoutMs`. */
async function bridgeAnswers(port: number, timeoutMs: number): Promise<boolean> {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/test/ready`, { signal: AbortSignal.timeout(timeoutMs) });
    return response.ok;
  } catch {
    return false;
  }
}

/** The end of the app's stderr that isn't a whole line yet: chunks split lines anywhere. */
let stderrPartial = '';

function recordStderr(chunk: Buffer): void {
  const lines = (stderrPartial + chunk.toString()).split('\n');
  stderrPartial = lines.pop()!;
  for (const line of lines) {
    const log = line.match(/^Devora Ember log: (\S+)$/);
    if (log) appLogPath = log[1];
  }
  appStderr.push(...lines);
  if (appStderr.length > STDERR_LINES) appStderr = appStderr.slice(-STDERR_LINES);
}

/** The app's latest stderr lines, its unfinished last line included. */
function stderrTail(count: number): string {
  return [...appStderr, ...(stderrPartial ? [stderrPartial] : [])].slice(-count).join('\n');
}

/** What the app said: its stderr since the last launch, and the end of the log it writes. */
function appEvidence(): string {
  let log = '(no log path seen on stderr)';
  if (appLogPath) {
    try {
      log = fs.readFileSync(appLogPath, 'utf8').split('\n').slice(-200).join('\n');
    } catch (error) {
      log = `(${appLogPath} unreadable: ${String(error)})`;
    }
  }
  return `== App stderr (since its last launch, latest ${STDERR_LINES} lines) ==\n${stderrTail(STDERR_LINES)}\n\n== ${appLogPath ?? 'App log'} (end) ==\n${log}\n`;
}

/** Saves the app's evidence beside a failure's screenshot, for CI's failure artifact. */
function captureAppEvidence(label: string): void {
  fs.mkdirSync(FAILURE_CAPTURE_DIR, { recursive: true });
  const file = path.join(FAILURE_CAPTURE_DIR, `${Date.now()}-${label.replace(/[^a-zA-Z0-9_-]+/g, '_').slice(0, 120)}.app.txt`);
  fs.writeFileSync(file, appEvidence());
  console.error(`Failure capture: ${file}`);
}

/**
 * Launches the app and waits until it can be driven, within one budget for all of it (the ports on stderr, the bridge, the frontend's test namespace).
 * A launch that runs out names the wait it was in, and its failure stops the rest of the run (see `launchFailure`).
 */
async function launchApp(): Promise<AppDriver> {
  const deadline = Date.now() + LAUNCH_BUDGET_MS;
  const left = () => Math.max(0, deadline - Date.now());
  const binary = findAppBinary();

  const ALLOWED_ENV_VARS = ['HOME', 'SHELL', 'TMPDIR', 'USER', 'LANG'] as const;
  const baseEnv: Record<string, string> = {};
  for (const key of ALLOWED_ENV_VARS) {
    if (process.env[key]) baseEnv[key] = process.env[key]!;
  }
  if (process.env.PATH) {
    baseEnv.PATH = process.env.PATH
      .split(':')
      .filter((p) => !p.includes('.app/Contents') && !p.includes('/Devora/'))
      .join(':');
  }

  appStderr = [];
  stderrPartial = '';
  appLogPath = null;
  const app = spawn(binary, [], {
    env: {
      ...baseEnv,
      DEVORA_TEST_MODE: '1',
      DEVORA_TEST_HEALTH_FIXTURE: path.join(testConfigDir!, 'health-report.json'),
      ANTHROPIC_BASE_URL: `http://127.0.0.1:${fakeClaudeServer!.getPort()}`,
      ANTHROPIC_API_KEY: 'sk-ant-fake-key-for-bdd-tests',
      DEVORA_CONFIG_PATH: testConfigPath!,
      DEVORA_STATE_DIR: path.join(testConfigDir!, 'state'),
      CLAUDE_CONFIG_DIR: CLAUDE_CONFIG_PATH,
      // No feature flags or telemetry from the network, so Claude Code behaves the same on every run.
      CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1',
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  appProcess = app;
  app.stderr!.on('data', recordStderr);

  let waitingFor = 'the ports on stderr';
  try {
    const ports = await new Promise<{ ipc: number; testHarness: number }>((resolve, reject) => {
      let foundIpcPort: number | null = null;
      let foundTestHarnessPort: number | null = null;
      const timeout = setTimeout(() => reject(new Error('no ports on stderr')), left());

      // A pipe delivers stderr in arbitrary chunks, so a line can arrive split in two ("...on port ", then "56325\n"):
      // match whole lines of everything received so far.
      let received = '';
      function onData(chunk: Buffer): void {
        received += chunk.toString();
        const ipcMatch = received.match(/Devora IPC server on port (\d+)\r?\n/);
        if (ipcMatch) foundIpcPort = parseInt(ipcMatch[1], 10);
        const harnessMatch = received.match(/Devora test harness on port (\d+)\r?\n/);
        if (harnessMatch) foundTestHarnessPort = parseInt(harnessMatch[1], 10);
        if (foundIpcPort !== null && foundTestHarnessPort !== null) {
          clearTimeout(timeout);
          app.stderr!.removeListener('data', onData);
          resolve({ ipc: foundIpcPort, testHarness: foundTestHarnessPort });
        }
      }

      app.stderr!.on('data', onData);
      app.on('error', (err) => {
        clearTimeout(timeout);
        reject(err);
      });
      app.on('exit', (code, signal) => {
        clearTimeout(timeout);
        reject(new Error(`the app exited (code ${code}, signal ${signal}) before its ports were found`));
      });
    });
    ipcPort = ports.ipc;
    testHarnessPort = ports.testHarness;

    waitingFor = 'its test bridge (/test/ready)';
    await waitForReady(testHarnessPort, left());

    // /test/ready only proves the harness HTTP server is up; the frontend may still be inside its DOMContentLoaded handler.
    // Wait until the app exposes its test namespace so the first scenario cannot race app initialization.
    waitingFor = 'its frontend (window.__test)';
    const driver = new AppDriver(testHarnessPort, ipcPort);
    await driver.pollFor("return typeof window.__test !== 'undefined'", true, left());
    return driver;
  } catch (error) {
    launchFailure = new Error(
      `The app's launch failed while waiting for ${waitingFor}, within its ${LAUNCH_BUDGET_MS / 1000} s budget: ` +
        `${error instanceof Error ? error.message : String(error)}\n` +
        `Its stderr and log are in the failure captures; latest stderr:\n${stderrTail(20)}`,
    );
    captureAppEvidence('launch-failed');
    throw launchFailure;
  }
}

/** Stops the app, if it still runs, and waits for its exit. */
async function stopApp(): Promise<void> {
  const previous = appProcess;
  if (previous && previous.exitCode === null && previous.signalCode === null) {
    const exited = new Promise<void>(resolve => previous.once('exit', () => resolve()));
    previous.kill();
    await exited;
  }
}

/** Whether a launch failed in this run: there is no app left to tear down. */
export function appLaunchFailed(): boolean {
  return launchFailure !== null;
}

/** Restart the real binary with the same config and state directory. */
export async function restartApp(): Promise<AppDriver> {
  await stopApp();
  return launchApp();
}

/**
 * Before a scenario: the shared app must still run and answer. If it doesn't, the app is launched again for the scenarios after it, and
 * this returns what happened, for this scenario to fail with (the run stays red, so a crash can't pass unnoticed). After a failed launch,
 * every scenario fails with that launch's error at once.
 */
async function appProblemBeforeScenario(): Promise<Error | null> {
  if (launchFailure) throw launchFailure;
  const app = appProcess!;
  const exitOf = () => `the app exited (code ${app.exitCode ?? 'none'}, signal ${app.signalCode ?? 'none'})`;
  let problem: string | null = null;
  if (app.exitCode !== null || app.signalCode !== null) {
    problem = exitOf();
  } else if (!(await bridgeAnswers(testHarnessPort!, 2_000))) {
    // A bridge that refuses at once may be an exit whose event hasn't arrived yet: give it a moment to tell which.
    const exited = await new Promise<boolean>((resolve) => {
      if (app.exitCode !== null || app.signalCode !== null) return resolve(true);
      const timer = setTimeout(() => resolve(false), 1_000);
      app.once('exit', () => {
        clearTimeout(timer);
        resolve(true);
      });
    });
    problem = exited ? exitOf() : "the app's test bridge didn't answer /test/ready within 2 s";
  }
  if (!problem) return null;
  const evidence = stderrTail(20);
  captureAppEvidence('app-gone-before-scenario');
  await stopApp();
  await launchApp();
  return new Error(`Before this scenario, ${problem}; it was launched again for the next scenarios. Latest stderr:\n${evidence}`);
}

BeforeAll({ timeout: LAUNCH_STEP_TIMEOUT_MS }, async function () {
  fs.rmSync(STRUCTURED_LOG_BASE_DIR, { recursive: true, force: true });
  fs.rmSync(FAILURE_CAPTURE_DIR, { recursive: true, force: true });

  fakeClaudeServer = new FakeClaudeServer();
  await fakeClaudeServer.start();

  testConfigDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-config-'));
  testConfigPath = path.join(testConfigDir, 'config.json');
  writeTestConfig(testConfigPath, []);

  fs.rmSync(CLAUDE_CONFIG_PATH, { recursive: true, force: true });
  fs.mkdirSync(path.join(CLAUDE_CONFIG_PATH, 'plans'), { recursive: true });
  fs.writeFileSync(
    path.join(CLAUDE_CONFIG_PATH, 'settings.json'),
    JSON.stringify({ permissions: { defaultMode: 'plan' }, theme: 'dark' }, null, 2),
  );
  // Claude Code shows one-time nudge dialogs (e.g. "Make auto mode your default permission mode?") depending on remote cohorts; one appearing mid-test swallows the keys meant for the prompt, so mark them as already seen.
  fs.writeFileSync(
    path.join(CLAUDE_CONFIG_PATH, '.claude.json'),
    JSON.stringify({ hasSeenAutoDefaultNudge: true, hasSeenEffortMediumNudge: true }, null, 2),
  );
  // Note: We intentionally do NOT copy .credentials.json here.
  // Tests use ANTHROPIC_API_KEY (pointed at the fake server) instead of OAuth tokens.
  // Copying credentials would cause an "auth conflict" warning in Claude Code.

  await launchApp();
});


AfterStep(async function (this: EmberWorld, { result, pickle, pickleStep }) {
  if (result.status !== Status.FAILED) return;
  await captureFailure(this.driver, `${pickle.name}--${pickleStep.text}`);
  captureAppEvidence(`${pickle.name}--${pickleStep.text}`);
});

Before({ timeout: LAUNCH_STEP_TIMEOUT_MS }, async function (this: EmberWorld, scenario) {
  const problem = await appProblemBeforeScenario();
  const healthFixture = path.join(testConfigDir!, 'health-report.json');
  if (scenario.pickle.tags.some((tag) => tag.name === '@real-health')) {
    fs.rmSync(healthFixture, { force: true });
  } else {
    fs.copyFileSync(path.resolve(import.meta.dirname, '../../../acceptance/fixtures/health-report.json'), healthFixture);
  }
  this.driver = new AppDriver(testHarnessPort!, ipcPort!);
  this.testConfigPath = testConfigPath!;
  // Failed only once its driver is on the relaunched app, so that the teardown runs on it.
  if (problem) throw problem;
});

Before({ tags: '@real-claude' }, async function (this: EmberWorld, scenario) {
  const scenarioName = scenario.pickle.name.replace(/[^a-zA-Z0-9_-]/g, '_');
  try {
    await fakeClaudeServer!.loadCassette(scenarioName);
  } catch (err: any) {
    if (err?.code === 'ENOENT' && process.env.RECORD_MODE !== '1') {
      return 'skipped';
    }
    throw err;
  }
});

After({ tags: '@real-claude' }, async function (this: EmberWorld) {
  if (this.stopAutoApprove) {
    this.stopAutoApprove();
    this.stopAutoApprove = undefined;
  }

  await fakeClaudeServer!.saveCassette();
  fakeClaudeServer!.reset();
});

// Locally, the terminal running the tests may lack macOS Screen Recording permission; CI must always run these.
Before({ tags: '@screen-capture' }, function () {
  if (!process.env.CI && !canCaptureScreen()) return 'skipped';
});

After(async function (this: EmberWorld, scenario) {
  if (launchFailure) return;
  // Cucumber runs After hooks in reverse registration order: verify CLI exit
  // here, before closing PTYs or deleting fixtures, rather than in the tagged hook.
  if (scenario.pickle.tags.some(tag => tag.name === '@real-claude')) {
    this.stopAutoApprove?.();
    this.stopAutoApprove = undefined;
    await stopClaudeCode(this.driver);
  }
  // A scenario can end while the app is still loading (e.g. a hub fetching settings); deleting the fixtures under a command in flight fails it and reports a backend error, so let the app settle first.
  await this.driver.pollFor(`return window.__test.invokesInFlight()`, 0, 5_000);
  if (this.fixtureRoot) {
    cleanupFixtures(this.fixtureRoot);
  }

  writeTestConfig(testConfigPath!, []);

  try {
    // Hard teardown of any open page layers (each runs its onCleanup, e.g. the hub's unload).
    await this.driver.eval(`window.__test.layers.clear();`);
  } catch {
    // app may not have fully loaded
  }

  // The app instance is shared across scenarios, and the Workspace Hub keeps the active profile path in memory across loads.
  // Reset it so a later scenario that reads it without reloading the hub (e.g. opening the Health Hub from the Command Palette) cannot inherit a now-deleted fixture's path.
  try {
    await this.driver.eval('window.__test.wsHub.setActiveProfilePath(null)');
  } catch {
    // app may not have fully loaded
  }

  try {
    await this.driver.eval(`
      for (const s of [...window.__test.sessionManager.getSessions()]) {
        window.__test.sessionManager.closeSession(s.id);
      }
    `);
  } catch {
    // app may not have fully loaded
  }

  // Font size is app-global and persists across scenarios, so a scenario that changes it would silently resize every later one's UI — enough to flip assertions about content overflowing its viewport.
  try {
    await this.driver.eval(`document.documentElement.style.fontSize = ''`);
  } catch {
    // app may not have fully loaded
  }

  if (this.workspacePath) {
    cleanupWorkspace(this.workspacePath);
  }

  // The app instance is shared across scenarios — leftover banners must not leak into the next scenario's DOM assertions.
  // (Clearing banners does not clear the scrape list, so the check below still catches unexpected errors.)
  try {
    await this.driver.eval('window.__test.clearErrorBanners?.()');
  } catch {
    // app may not have fully loaded
  }

  if (this.driver) {
    const errors = await this.driver.eval('return window.__scrapeErrors ? window.__scrapeErrors() : []');
    if (errors && errors.length > 0) {
      throw new Error(`Backend errors during scenario:\n${errors.map((e: string) => `  - ${e}`).join('\n')}`);
    }
  }
});

AfterAll(async function () {
  if (appProcess) {
    appProcess.kill();
    appProcess = null;
  }
  if (fakeClaudeServer) {
    await fakeClaudeServer.stop();
    fakeClaudeServer = null;
  }
  if (testConfigDir) {
    fs.rmSync(testConfigDir, { recursive: true, force: true });
    testConfigDir = null;
    testConfigPath = null;
  }
  fs.rmSync(CLAUDE_CONFIG_PATH, { recursive: true, force: true });
});
