import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import { logToFile, showError } from './errors';

export { Channel } from '@tauri-apps/api/core';

let inFlight = 0;

/** Commands invoked but not yet settled; the acceptance-test teardown waits for zero before deleting fixtures. */
export function invokesInFlight(): number {
  return inFlight;
}

async function tracked<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  inFlight++;
  try {
    return await tauriInvoke<T>(cmd, args);
  } finally {
    inFlight--;
  }
}

/** The app's default invoke: a rejected command is surfaced to the user via showError (banner + record + log), then rethrown for caller flow control. */
export async function invoke<T = unknown>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tracked<T>(cmd, args);
  } catch (e) {
    showError(`${cmd} failed: ${String(e)}`);
    throw e;
  }
}

/** Explicit opt-out for high-frequency or gracefully-degrading commands: a rejection is written to the log file as WARN (no banner, no recorded error), then rethrown.
 * Fire-and-forget callers must append .catch(() => {}) so the rejection cannot re-surface via the unhandledrejection handler. */
export async function invokeLogOnly<T = unknown>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tracked<T>(cmd, args);
  } catch (e) {
    logToFile('WARN', `${cmd} failed: ${String(e)}`);
    throw e;
  }
}
