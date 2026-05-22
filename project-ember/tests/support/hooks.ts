import { Before, After, BeforeAll, AfterAll } from '@cucumber/cucumber';
import { spawn, ChildProcess } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { AppDriver } from './app-driver';
import { FakeClaudeServer, STRUCTURED_LOG_BASE_DIR } from './fake-claude-server';
import { cleanupWorkspace, stopClaudeCode } from './claude-helper';
import { assertOriginalCritAvailable } from './crit-helper';
import { cleanupFixtures, writeTestConfig } from './fixture-helper';
import { EmberWorld } from './world';

const PROJECT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

let appProcess: ChildProcess | null = null;
let testHarnessPort: number | null = null;
let ipcPort: number | null = null;
let fakeClaudeServer: FakeClaudeServer | null = null;
let testConfigPath: string | null = null;
let testConfigDir: string | null = null;
const CLAUDE_CONFIG_PATH = '/tmp/ember-bdd-claude';

function findAppBundleBinary(bundleDir: string): string | null {
  if (!fs.existsSync(bundleDir)) return null;
  const appDirs = fs.readdirSync(bundleDir).filter((e) => e.endsWith('.app'));
  if (appDirs.length === 0) return null;
  if (appDirs.length > 1) {
    throw new Error(
      `Multiple .app bundles found in ${bundleDir}: ${appDirs.join(', ')}. ` +
        'Delete stale bundles and rebuild.',
    );
  }
  const binary = path.join(bundleDir, appDirs[0], 'Contents', 'MacOS', 'devora-ember');
  return fs.existsSync(binary) ? binary : null;
}

function findAppBinary(): string {
  // Prefer bundled .app (has bundled-apps/ directory)
  const bundleSearchDirs = [
    path.resolve(PROJECT_ROOT, '../../bin/macOS-dev'),
    path.join(PROJECT_ROOT, 'src-tauri/target/release/bundle/macos'),
  ];
  for (const dir of bundleSearchDirs) {
    const binary = findAppBundleBinary(dir);
    if (binary) return binary;
  }

  // Fall back to raw binaries (no bundled-apps/ available)
  const releasePath = path.join(PROJECT_ROOT, 'src-tauri/target/release/devora-ember');
  const debugPath = path.join(PROJECT_ROOT, 'src-tauri/target/debug/devora-ember');
  if (fs.existsSync(releasePath)) return releasePath;
  if (fs.existsSync(debugPath)) return debugPath;
  throw new Error(
    `App binary not found. Searched for .app bundles in [${bundleSearchDirs.join(', ')}] ` +
      `and raw binaries at ${releasePath}, ${debugPath}. Run "cargo tauri build" first.`,
  );
}

async function waitForReady(port: number, timeoutMs: number): Promise<void> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/test/ready`);
      if (response.ok) return;
    } catch {
      // not ready yet
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`App did not become ready within ${timeoutMs}ms`);
}

BeforeAll(async function () {
  fs.rmSync(STRUCTURED_LOG_BASE_DIR, { recursive: true, force: true });

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
  // Note: We intentionally do NOT copy .credentials.json here.
  // Tests use ANTHROPIC_API_KEY (pointed at the fake server) instead of
  // OAuth tokens.  Copying credentials would cause an "auth conflict"
  // warning in Claude Code.

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

  appProcess = spawn(binary, [], {
    env: {
      ...baseEnv,
      DEVORA_TEST_MODE: '1',
      ANTHROPIC_BASE_URL: `http://127.0.0.1:${fakeClaudeServer.getPort()}`,
      ANTHROPIC_API_KEY: 'sk-ant-fake-key-for-bdd-tests',
      DEVORA_CONFIG_PATH: testConfigPath,
      CLAUDE_CONFIG_DIR: CLAUDE_CONFIG_PATH,
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  const portsPromise = new Promise<{ ipc: number; testHarness: number }>((resolve, reject) => {
    let foundIpcPort: number | null = null;
    let foundTestHarnessPort: number | null = null;

    const timeout = setTimeout(
      () => reject(new Error('Timed out waiting for ports on stderr')),
      30_000,
    );

    function tryResolve(): void {
      if (foundIpcPort !== null && foundTestHarnessPort !== null) {
        clearTimeout(timeout);
        appProcess!.stderr!.removeListener('data', onData);
        resolve({ ipc: foundIpcPort, testHarness: foundTestHarnessPort });
      }
    }

    function onData(chunk: Buffer): void {
      const text = chunk.toString();

      const ipcMatch = text.match(/Devora IPC server on port (\d+)/);
      if (ipcMatch) {
        foundIpcPort = parseInt(ipcMatch[1], 10);
      }

      const harnessMatch = text.match(/Devora test harness on port (\d+)/);
      if (harnessMatch) {
        foundTestHarnessPort = parseInt(harnessMatch[1], 10);
      }

      tryResolve();
    }

    appProcess!.stderr!.on('data', onData);
    appProcess!.on('error', (err) => {
      clearTimeout(timeout);
      reject(err);
    });
    appProcess!.on('exit', (code) => {
      clearTimeout(timeout);
      reject(new Error(`App exited with code ${code} before ports were found`));
    });
  });

  const ports = await portsPromise;
  ipcPort = ports.ipc;
  testHarnessPort = ports.testHarness;

  await waitForReady(testHarnessPort, 30_000);
});

Before(async function (this: EmberWorld) {
  this.driver = new AppDriver(testHarnessPort!, ipcPort!);
  this.testConfigPath = testConfigPath!;
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
  await stopClaudeCode(this.driver);

  try {
    await this.driver.eval(`
      const sessions = window.__test.sessionManager.getSessions();
      for (const s of [...sessions]) {
        window.__test.overlayManager.dismissPanelOverlay(s.id);
      }
    `);
  } catch {
    // overlay may not be active
  }

  await fakeClaudeServer!.saveCassette();
  fakeClaudeServer!.reset();
});

Before({ tags: '@real-crit' }, async function (this: EmberWorld) {
  assertOriginalCritAvailable();
});

After(async function (this: EmberWorld) {
  if (this.fixtureRoot) {
    cleanupFixtures(this.fixtureRoot);
  }
  if (this.bareRepoPath) {
    fs.rmSync(this.bareRepoPath, { recursive: true, force: true });
  }

  writeTestConfig(testConfigPath!, []);

  try {
    await this.driver.eval(`
      if (window.__test.overlayManager.isTabCoveringOverlayActive()) {
        window.__test.wsHub.unload();
        window.__test.overlayManager.dismissTabCoveringOverlay();
      }
    `);
  } catch {
    // app may not have fully loaded
  }

  try {
    await this.driver.eval(`
      const sessions = window.__test.sessionManager.getSessions();
      for (const s of [...sessions]) {
        window.__test.overlayManager.dismissPanelOverlay(s.id);
      }
      for (const s of [...sessions]) {
        window.__test.sessionManager.closeSession(s.id);
      }
    `);
  } catch {
    // app may not have fully loaded
  }

  if (this.workspacePath) {
    cleanupWorkspace(this.workspacePath);
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
