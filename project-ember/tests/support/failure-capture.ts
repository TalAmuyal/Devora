import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { AppDriver } from './app-driver';
import { getTerminalContent } from './terminal-helper';

/** Where failed steps leave their evidence; CI uploads this directory as an artifact when the job fails. */
export const FAILURE_CAPTURE_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../test-results/failures');

export interface FailureCapture {
  screenshot?: string;
  terminal?: string;
}

/**
 * Record what the app looked like when a step failed: a screenshot of the screen and the active terminal's text.
 * Captured at the moment of failure, before teardown changes the UI. Best effort: a capture that fails is skipped.
 */
export async function captureFailure(
  driver: AppDriver,
  label: string,
  dir = FAILURE_CAPTURE_DIR,
): Promise<FailureCapture> {
  fs.mkdirSync(dir, { recursive: true });
  const base = path.join(dir, `${Date.now()}-${label.replace(/[^a-zA-Z0-9_-]+/g, '_').slice(0, 120)}`);
  const capture: FailureCapture = {};

  try {
    execFileSync('screencapture', ['-x', `${base}.png`], { timeout: 10_000, stdio: 'pipe' });
    capture.screenshot = `${base}.png`;
  } catch (error) {
    console.error(`Failure screenshot skipped: ${String(error)}`);
  }

  try {
    const terminal = await getTerminalContent(driver);
    if (terminal.trim()) {
      fs.writeFileSync(`${base}.terminal.txt`, terminal.trimEnd() + '\n');
      capture.terminal = `${base}.terminal.txt`;
    }
  } catch (error) {
    console.error(`Failure terminal capture skipped: ${String(error)}`);
  }

  for (const file of Object.values(capture)) {
    console.error(`Failure capture: ${file}`);
  }
  return capture;
}

/** Whether this machine lets the test process capture the screen (macOS Screen Recording permission). */
export function canCaptureScreen(): boolean {
  const probe = path.join(os.tmpdir(), `ember-screen-probe-${process.pid}.png`);
  try {
    execFileSync('screencapture', ['-x', probe], { timeout: 10_000, stdio: 'ignore' });
    return fs.existsSync(probe);
  } catch {
    return false;
  } finally {
    fs.rmSync(probe, { force: true });
  }
}
