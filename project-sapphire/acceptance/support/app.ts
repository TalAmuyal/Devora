/** Launching, quitting and killing the app under test. */

import { execFileSync, spawn, type ChildProcess } from 'node:child_process';
import * as path from 'node:path';
import { Bridge, type AppState } from './bridge';

const PROJECT_DIR = path.resolve(import.meta.dirname, '../..');
export const APP_BINARY = path.join(path.resolve(PROJECT_DIR, process.env.SAPPHIRE_APP!), 'Contents/MacOS/Devora Sapphire');
export const BUNDLED_APP_BINARY = path.join(PROJECT_DIR, 'build/bundle/Devora Sapphire.app/Contents/MacOS/Devora Sapphire');
/** The core's deterministic fake Claude CLI, which agents run instead of Claude (`devora-core`'s `agents/launch.rs`). */
const FAKE_CLAUDE = path.resolve(PROJECT_DIR, '../project-core/crates/devora-agent/tests/support/fake_cli.py');
const FAKE_CLAUDE_FIXTURES = path.resolve(PROJECT_DIR, '../project-core/crates/devora-agent/tests/fixtures/claude-2.1.312');

/** The app's stderr, per process, for failure captures. */
export const stderrOf = new WeakMap<ChildProcess, string[]>();

export interface LaunchedApp {
  app: ChildProcess;
  bridge: Bridge;
}

/**
 * Launches the app on a scenario's config and state directory, and returns once its test bridge is up.
 *
 * The bridge comes up once the launch is done, or early when the app state couldn't be opened, so that the scenario can answer the startup problem.
 */
export async function launch(configPath: string, stateDir: string | undefined, options: { binary?: string; home?: string; environment?: Record<string, string>; arguments?: string[] } = {}): Promise<LaunchedApp> {
  // Only what a login session would provide, so the developer's environment (and a Devora app around the test run) doesn't leak into the app's terminals.
  const env: Record<string, string> = {};
  for (const key of ['HOME', 'SHELL', 'TMPDIR', 'USER', 'LANG']) {
    if (process.env[key]) env[key] = process.env[key]!;
  }
  env.PATH = (process.env.PATH ?? '')
    .split(':')
    .filter((p) => !p.includes('.app/Contents') && !p.includes('/Devora/'))
    .join(':');
  if (options.home) env.HOME = options.home;

  // Launched directly rather than through `open`, so the harness owns the process and its stderr; Sapphire takes no bare arguments, so a window opens.
  // `arguments` are `-Default value` pairs, which AppKit reads as user defaults for this launch only.
  const app = spawn(options.binary ?? APP_BINARY, options.arguments ?? [], {
    env: {
      ...env,
      DEVORA_TEST_CLAUDE_PROGRAM: FAKE_CLAUDE,
      DEVORA_FAKE_FIXTURES: FAKE_CLAUDE_FIXTURES,
      DEVORA_FAKE_REPORT: path.join(path.dirname(configPath), 'fake-claude.jsonl'),
      ...options.environment,
      DEVORA_TEST_MODE: '1', DEVORA_TEST_HEALTH_FIXTURE: path.join(path.dirname(configPath), 'health-report.json'), DEVORA_CONFIG_PATH: configPath, ...(stateDir ? { DEVORA_STATE_DIR: stateDir } : {}) },
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
  return { app, bridge: new Bridge(port) };
}

/** Waits until the app state is open and either every restored tab that runs a terminal runs it, or, when none was restored, the Workspace Hub the launch opens has loaded. */
export function waitUntilReady(bridge: Bridge): Promise<AppState> {
  // The bridge starts once the launch is done, but restored tabs reach the store's state on a later frame; steps that read the state before acting need it settled.
  return bridge.waitFor(
    'the launch to settle',
    () => bridge.state(),
    (s) =>
      s.startupProblem === null &&
      (s.tabs.length > 0 || (s.hub !== null && !s.hub.loading)) &&
      s.tabs.every((tab) => tab.terminal === 'running' || !runsTerminal(tab.kind)),
  );
}

function runsTerminal(kind: string): boolean {
  return kind === 'shell' || kind === 'workspace';
}

export function hasExited(app: ChildProcess): boolean {
  return app.exitCode !== null || app.signalCode !== null;
}

/** Resolves with the exit code (or signal) once the app has exited. */
function exited(app: ChildProcess, timeoutMs = 10_000): Promise<number | NodeJS.Signals> {
  if (hasExited(app)) return Promise.resolve(app.exitCode ?? app.signalCode!);
  return new Promise((resolve, reject) => {
    const onExit = (code: number | null, signal: NodeJS.Signals | null) => {
      clearTimeout(timer);
      resolve(code ?? signal!);
    };
    const timer = setTimeout(() => {
      app.off('exit', onExit);
      reject(new Error(`The app did not exit within ${timeoutMs}ms`));
    }, timeoutMs);
    app.once('exit', onExit);
  });
}

/** Quits the app as the app menu's Quit does, and waits for it to exit. */
export async function quit(app: LaunchedApp): Promise<void> {
  const [, result] = await Promise.all([app.bridge.quit(), exited(app.app)]);
  if (result !== 0) throw new Error(`The app exited with ${result} on quit:\n${(stderrOf.get(app.app) ?? []).join('')}`);
}

/** Sends `signal` and waits for the app to exit; does nothing if it already has. */
export async function stop(app: ChildProcess, signal: NodeJS.Signals, timeoutMs = 10_000): Promise<void> {
  if (hasExited(app)) return;
  const done = exited(app, timeoutMs);
  app.kill(signal);
  await done;
}

interface AgentChild {
  pid: number;
  parent: number;
  command: string;
}

/** The full command includes the CLI session ID, guarding against PID reuse. */
function agentProcesses(): AgentChild[] {
  const rows = execFileSync('ps', ['-axo', 'pid=,ppid=,command='], { encoding: 'utf8' });
  return rows.split('\n').flatMap(line => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.+)$/);
    return match && match[3].includes(FAKE_CLAUDE)
      ? [{ pid: Number(match[1]), parent: Number(match[2]), command: match[3] }] : [];
  });
}

async function waitForAgentChildren(children: AgentChild[]): Promise<void> {
  const deadline = Date.now() + 3000;
  let remaining = children;
  while (remaining.length > 0) {
    const current = agentProcesses();
    remaining = remaining.filter(child => current.some(process => process.pid === child.pid && process.command === child.command));
    if (remaining.length === 0) return;
    if (Date.now() >= deadline) {
      throw new Error(`Agent children survived app shutdown: ${remaining.map(child => child.pid).join(', ')}`);
    }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
}

/** Normal Quit reaps agents before fixtures disappear. A forced fallback still
 * waits for the owned fake CLI children, and preserves the shutdown failure. */
export async function shutdown(app: LaunchedApp): Promise<void> {
  const children = agentProcesses().filter(child => child.parent === app.app.pid);
  try {
    if (!hasExited(app.app)) await quit(app);
  } finally {
    if (!hasExited(app.app)) {
      children.push(...agentProcesses().filter(child => child.parent === app.app.pid));
      try { await stop(app.app, 'SIGTERM', 3000); }
      catch { await stop(app.app, 'SIGKILL', 3000); }
    }
    await waitForAgentChildren(children);
  }
}
