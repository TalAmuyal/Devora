// The hang watchdog's thread (hang-watchdog.ts): it runs while the main thread is blocked, so it reports and stops a run
// that can't report or stop itself. Plain JavaScript, so it doesn't depend on the main thread's TypeScript loader.
import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { parentPort, workerData } from 'node:worker_threads';

const { limitSeconds, report } = workerData;
const verdictFile = report.replace(/\.txt$/, '') + '.json';
const OUTPUT_TAIL = 64 * 1024;
const RECENT = 20;
const GRACE_MS = 10_000;
// How long each piece of evidence may take: a command past it is killed (SIGKILL, which it can't ignore) and left behind,
// so that no evidence command, however stuck, keeps the run from being stopped.
const PS_SECONDS = 10;
const SAMPLE_SECONDS = 20;
const SCREENSHOT_SECONDS = 10;
const LOG_SECONDS = 20;

/** The latest activities, oldest first, each `{ at, what }`. */
const recent = [];
const outputs = new Map();
/** The scenario running, as the flake report knows it (`{ suite, name }`, cucumber.sh's identity), or null between scenarios. */
let scenario = null;
const startedAt = new Date();

parentPort.on('message', (message) => {
  if (message.activity !== undefined) {
    recent.push({ at: Date.now(), what: message.activity });
    if (recent.length > RECENT) recent.shift();
    if (message.scenario !== undefined) scenario = message.scenario;
  } else {
    outputs.set(message.label, ((outputs.get(message.label) ?? '') + message.chunk).slice(-OUTPUT_TAIL));
  }
});

// Once, so that a log without it shows the watchdog never started (rather than started and stayed silent).
// Straight to the file descriptor: the worker's process.stderr goes through the main thread, which may be blocked.
fs.writeSync(2, `acceptance: the hang watchdog is watching (it stops the run after ${limitSeconds} s without progress)\n`);

const timer = setInterval(() => {
  const latest = recent.at(-1);
  if (latest && Date.now() - latest.at >= limitSeconds * 1000) {
    clearInterval(timer);
    void stopTheRun(latest);
  }
}, 1000);

/**
 * Runs a command for its output, for at most `seconds`: past that it's killed with SIGKILL and not waited for, since a
 * process stuck in a call to WindowServer may never exit. Resolves to `{ stdout, finished }`.
 */
function bounded(command, args, seconds) {
  return new Promise((resolve) => {
    let stdout = '';
    let child;
    try {
      child = spawn(command, args, { stdio: ['ignore', 'pipe', 'ignore'] });
    } catch (error) {
      resolve({ stdout: '', finished: false, error: error.message });
      return;
    }
    child.stdout.on('data', (chunk) => { stdout += chunk; });
    const deadline = setTimeout(() => {
      child.kill('SIGKILL');
      child.stdout.destroy();
      resolve({ stdout, finished: false });
    }, seconds * 1000);
    child.on('error', (error) => {
      clearTimeout(deadline);
      resolve({ stdout, finished: false, error: error.message });
    });
    child.on('close', (code) => {
      clearTimeout(deadline);
      resolve({ stdout, finished: code === 0, error: code === 0 ? undefined : `exit ${code}` });
    });
  });
}

/** Why a command gave no evidence, in the report's words. */
function missing(what, seconds, result) {
  return result.error ? `(${what} failed: ${result.error})` : `(${what} didn't finish within ${seconds} s, and was killed)`;
}

const PS = ['-axo', 'pid=,ppid=,etime=,command='];

/** Every process but the `ps` listing them, or null when `ps` didn't answer. */
async function processes() {
  const result = await bounded('ps', PS, PS_SECONDS);
  if (!result.finished) return null;
  return result.stdout.split('\n').filter(Boolean).map((row) => {
    const [, pid, ppid, etime, command] = row.match(/^\s*(\d+)\s+(\d+)\s+(\S+)\s(.*)$/);
    return { pid: Number(pid), ppid: Number(ppid), etime, command };
  }).filter((p) => p.command !== `ps ${PS.join(' ')}`);
}

function descendantsOf(pid, all) {
  const children = all.filter((p) => p.ppid === pid);
  return children.flatMap((child) => [child, ...descendantsOf(child.pid, all)]);
}

/** The nearest few, which show what started the run (in CI: mise, through with-ui-lock.sh). */
function ancestorsOf(pid, all, levels = 3) {
  const parent = all.find((p) => p.pid === all.find((q) => q.pid === pid)?.ppid);
  return parent && parent.pid > 1 && levels > 0 ? [...ancestorsOf(parent.pid, all, levels - 1), parent] : [];
}

function describe(rows) {
  return rows.map((p) => `${String(p.pid).padStart(6)} ${String(p.ppid).padStart(6)} ${p.etime.padStart(11)}  ${p.command}`).join('\n') || '(none)';
}

/** The local time `log show --start` takes. */
function logTime(date) {
  const pad = (n) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

/**
 * Names the hang first, then gathers what it can, each piece appended to the report as it comes, then stops the run: a
 * stuck evidence command can cost the evidence, never the name or the stop.
 */
async function stopTheRun(latest) {
  const seconds = Math.round((Date.now() - latest.at) / 1000);
  fs.mkdirSync(path.dirname(report), { recursive: true });
  const append = (section) => fs.appendFileSync(report, `\n${section}\n`);
  fs.writeFileSync(report, [
    `# No progress for ${seconds} s since ${latest.what}`,
    `## The latest activity, oldest first\n${recent.map((a) => `${new Date(a.at).toISOString()}  ${a.what}`).join('\n')}`,
  ].join('\n\n') + '\n');
  fs.writeSync(2, `\nacceptance: no progress for ${seconds} s since ${latest.what}; stopping the run. What was running, and the main thread's stack: ${report}\n`);

  const all = await processes();
  const descendants = all ? descendantsOf(process.pid, all) : [];
  if (all) {
    const run = all.find((p) => p.pid === process.pid);
    append(`## The run, under its ancestors (pid, ppid, elapsed, command)\n${describe([...ancestorsOf(process.pid, all), ...(run ? [run] : [])])}`);
    append(`## What the run started, still running\n${describe(descendants)}`);
  } else {
    append(`## Processes\n(ps didn't answer within ${PS_SECONDS} s, so what the run started is left running)`);
  }

  // The main thread's native stack shows whether it's blocked in a synchronous spawn (node::SyncProcessRunner).
  // Before the screenshot: `sample` doesn't need WindowServer, while a screenshot waits for it.
  const sampleFile = `${report}.sample`;
  const sample = await bounded('sample', [String(process.pid), '3', '-file', sampleFile], SAMPLE_SECONDS);
  append(`## The run's threads (sample, 3 s)\n${sample.finished ? fs.readFileSync(sampleFile, 'utf8').split('\nBinary Images:')[0] : missing('sample', SAMPLE_SECONDS, sample)}`);
  fs.rmSync(sampleFile, { force: true });

  const screenshot = report.replace(/\.txt$/, '') + '.png';
  const capture = await bounded('screencapture', ['-x', screenshot], SCREENSHOT_SECONDS);
  // As the UI test watchdog says it (project-sapphire/scripts/ui-test-watchdog.sh).
  const unanswered = !capture.finished && !capture.error ? `WindowServer didn't answer within ${SCREENSHOT_SECONDS}s` : '';
  append(`## Screenshot\n${capture.finished ? screenshot : unanswered ? `no screenshot: ${unanswered}` : missing('screencapture', SCREENSHOT_SECONDS, capture)}`);

  // The UI test watchdog's rule (project-sapphire/UITests/CLAUDE.md): the runner, not the test, when WindowServer logged
  // GPU fence timeouts since the run started and didn't answer when the watchdog fired.
  const fences = await bounded('log', ['show', '--start', logTime(startedAt), '--style', 'compact',
    '--predicate', 'process == "WindowServer" AND eventMessage CONTAINS "timed out fence"'], LOG_SECONDS);
  const fenceLines = fences.stdout.split('\n').filter((line) => line.includes('timed out fence'));
  const firstFence = fenceLines[0]?.match(/^\S+ \S+/)?.[0] ?? null;
  const verdict = unanswered && fenceLines.length > 0 ? 'runner' : 'test';
  append(`## Verdict: ${verdict}\n${verdict === 'runner'
    ? `Runner problem, not the test: WindowServer stopped answering (${fenceLines.length} "timed out fence" lines from ${firstFence}; ${unanswered})`
    : `WindowServer ${unanswered ? `didn't answer (${unanswered}), but logged no GPU fence timeouts` : 'answered'}${fences.finished ? '' : ` (${missing('log show', LOG_SECONDS, fences)})`}`}`);
  fs.writeFileSync(verdictFile, JSON.stringify({
    suite: scenario?.suite ?? null,
    name: scenario?.name ?? null,
    activity: latest.what,
    verdict,
    fences_from: firstFence,
    windowserver_unanswered: unanswered || null,
  }) + '\n');

  for (const [label, text] of outputs) append(`## ${label} (its end)\n${text}`);

  // The processes it started first: the run may be blocked waiting for one, and they mustn't outlive it.
  await terminate(descendants.map((p) => p.pid));
  await terminate([process.pid]);
}

function alive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** SIGTERM, then SIGKILL for those still there after the grace: a blocked main thread can't run a SIGTERM handler. */
async function terminate(pids) {
  for (const pid of pids) {
    try { process.kill(pid, 'SIGTERM'); } catch { /* already gone */ }
  }
  const deadline = Date.now() + GRACE_MS;
  while (pids.some(alive) && Date.now() < deadline) await sleep(100);
  for (const pid of pids.filter(alive)) {
    try { process.kill(pid, 'SIGKILL'); } catch { /* already gone */ }
  }
}
