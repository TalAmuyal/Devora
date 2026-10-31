/** Launching, quitting and killing the app under test. */

import { spawn, type ChildProcess } from 'node:child_process';
import * as path from 'node:path';
import { Bridge, type AppState } from './bridge';

const PROJECT_DIR = path.resolve(import.meta.dirname, '../..');
export const APP_BINARY = path.join(PROJECT_DIR, process.env.SAPPHIRE_APP!, 'Contents/MacOS/Devora Sapphire');

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
export async function launch(configPath: string, stateDir: string): Promise<LaunchedApp> {
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
    env: { ...env, DEVORA_TEST_MODE: '1', DEVORA_CONFIG_PATH: configPath, DEVORA_STATE_DIR: stateDir },
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

/** Waits until the app state is open and every tab that runs a terminal runs it; the launch opens a shell tab when it restored none. */
export function waitUntilReady(bridge: Bridge): Promise<AppState> {
  // The bridge starts once the launch is done, but its tabs reach the store's state on a later frame; steps that read the state before acting need it settled.
  return bridge.waitFor(
    'the launch to settle with its tabs running',
    () => bridge.state(),
    (s) =>
      s.startupProblem === null &&
      s.tabs.length > 0 &&
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
function exited(app: ChildProcess): Promise<number | NodeJS.Signals> {
  if (hasExited(app)) return Promise.resolve(app.exitCode ?? app.signalCode!);
  return new Promise((resolve) => app.once('exit', (code, signal) => resolve(code ?? signal!)));
}

/** Quits the app as the app menu's Quit does, and waits for it to exit. */
export async function quit(app: LaunchedApp): Promise<void> {
  const done = exited(app.app);
  await app.bridge.quit();
  const result = await done;
  if (result !== 0) throw new Error(`The app exited with ${result} on quit:\n${(stderrOf.get(app.app) ?? []).join('')}`);
}

/** Sends `signal` and waits for the app to exit; does nothing if it already has. */
export async function stop(app: ChildProcess, signal: NodeJS.Signals): Promise<void> {
  if (hasExited(app)) return;
  const done = exited(app);
  app.kill(signal);
  await done;
}
