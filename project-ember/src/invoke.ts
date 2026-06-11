import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/core';
import { logToFile, showError } from './errors';

export { Channel } from '@tauri-apps/api/core';

/** The app's default invoke: a rejected command is surfaced to the user via showError (banner + record + log), then rethrown for caller flow control. */
export async function invoke<T = unknown>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    showError(`${cmd} failed: ${String(e)}`);
    throw e;
  }
}

/** Explicit opt-out for high-frequency or gracefully-degrading commands: a rejection is written to the log file as WARN (no banner, no recorded error), then rethrown.
 * Fire-and-forget callers must append .catch(() => {}) so the rejection cannot re-surface via the unhandledrejection handler. */
export async function invokeLogOnly<T = unknown>(cmd: string, args?: InvokeArgs): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (e) {
    logToFile('WARN', `${cmd} failed: ${String(e)}`);
    throw e;
  }
}
