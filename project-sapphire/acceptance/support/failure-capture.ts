import * as fs from 'node:fs';
import * as path from 'node:path';
import { stderrOf } from './app';
import type { SapphireWorld } from './world';

/** Where failed steps leave their evidence; CI uploads this directory as an artifact when the job fails. */
export const FAILURE_CAPTURE_DIR = path.resolve(import.meta.dirname, '../../test-results/failures');

/** The files a failure capture wrote. */
export interface FailureCapture {
  screenshot?: string;
  text: string;
}

/** Saves a window screenshot, the app's state, the active terminal's text and the app's stderr to `test-results/failures/`; in CI, they are uploaded with the failed run. */
export async function captureFailure(world: SapphireWorld, label: string, dir = FAILURE_CAPTURE_DIR): Promise<FailureCapture> {
  fs.mkdirSync(dir, { recursive: true });
  const base = path.join(dir, label.replace(/[^a-zA-Z0-9_-]+/g, '_').slice(0, 150));
  const capture: FailureCapture = { text: `${base}.txt` };
  const sections: string[] = [];
  try {
    // ScreenCaptureKit captures the app's own window, with no Screen Recording permission; a failed capture is only logged.
    await world.bridge.snapshot(`${base}.png`);
    if (fs.existsSync(`${base}.png`)) capture.screenshot = `${base}.png`;
  } catch (error) {
    sections.push(`## Screenshot error\n${error}`);
  }
  try {
    sections.push(`## State\n${JSON.stringify(await world.bridge.state(), null, 2)}`);
    sections.push(`## Active terminal\n${await world.bridge.terminalText()}`);
  } catch (error) {
    sections.push(`## Bridge error\n${error}`);
  }
  sections.push(`## stderr\n${(stderrOf.get(world.app) ?? []).join('')}`);
  if (world.second) sections.push(`## Second app's stderr\n${(stderrOf.get(world.second.app) ?? []).join('')}`);
  fs.writeFileSync(capture.text, sections.join('\n\n') + '\n');
  return capture;
}
