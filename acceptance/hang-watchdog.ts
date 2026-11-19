import * as path from 'node:path';
import { Worker } from 'node:worker_threads';

/**
 * Names a hang that Cucumber's step timeouts can't: one that blocks the main thread (a synchronous spawn that never
 * returns stops every timer, Cucumber's included), or one outside any step (a hook without a timeout, a process that
 * doesn't exit after the last scenario).
 * It watches from a worker thread, which a blocked main thread doesn't stop. When nothing has started or finished for
 * `ACCEPTANCE_HANG_LIMIT_SECONDS` (300 by default; the longest step timeout is 120 s), it writes what was running, the
 * processes around the run, the main thread's stack, a screenshot and the launched processes' output to
 * `ACCEPTANCE_HANG_REPORT` (test-results/failures/hang.txt, which CI uploads with the failure captures), then stops the
 * run and the processes it started.
 * `hang-watchdog-formatter.ts` reports the run's progress to it; a harness reports what its launched processes print.
 */
export interface HangWatchdog {
  /** Something started or finished: the wait starts over, and a hang names the latest. */
  activity(what: string): void;
  /** Output of a process the harness launched, such as the app's stderr: the report ends with the end of each. */
  output(label: string, chunk: string): void;
}

// One for the formatter and the harness alike: package.json makes this directory ESM, so tsx loads this file once.
// (Without it, tsx loaded it as CommonJS too, a second copy with a watchdog of its own.)
let watchdog: HangWatchdog | undefined;

/** The run's one watchdog; it watches from the first activity (the formatter's start), so a run without the formatter is never stopped. */
export function hangWatchdog(): HangWatchdog {
  if (!watchdog) {
    const worker = new Worker(new URL('./hang-watchdog-worker.mjs', import.meta.url), {
      workerData: {
        limitSeconds: Number(process.env.ACCEPTANCE_HANG_LIMIT_SECONDS ?? 300),
        report: path.resolve(process.env.ACCEPTANCE_HANG_REPORT ?? 'test-results/failures/hang.txt'),
      },
    });
    // It never keeps a run that finished from exiting: one that doesn't exit on its own is what it reports.
    worker.unref();
    watchdog = {
      activity: (what) => worker.postMessage({ activity: what }),
      output: (label, chunk) => worker.postMessage({ label, chunk }),
    };
  }
  return watchdog;
}
