// The hang watchdog's thread (hang-watchdog.ts): it runs while the main thread is blocked, so it reports and stops a run
// that can't report or stop itself. Plain JavaScript, so it doesn't depend on the main thread's TypeScript loader.
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { setTimeout as sleep } from 'node:timers/promises';
import { parentPort, workerData } from 'node:worker_threads';

const { limitSeconds, report } = workerData;
const OUTPUT_TAIL = 64 * 1024;
const RECENT = 20;
const GRACE_MS = 10_000;

/** The latest activities, oldest first, each `{ at, what }`. */
const recent = [];
const outputs = new Map();

parentPort.on('message', (message) => {
  if (message.activity !== undefined) {
    recent.push({ at: Date.now(), what: message.activity });
    if (recent.length > RECENT) recent.shift();
  } else {
    outputs.set(message.label, ((outputs.get(message.label) ?? '') + message.chunk).slice(-OUTPUT_TAIL));
  }
});

const timer = setInterval(() => {
  const latest = recent.at(-1);
  if (latest && Date.now() - latest.at >= limitSeconds * 1000) {
    clearInterval(timer);
    void stopTheRun(latest);
  }
}, 1000);

const PS = ['-axo', 'pid=,ppid=,etime=,command='];

/** Every process but the `ps` listing them. */
function processes() {
  const rows = execFileSync('ps', PS, { encoding: 'utf8' });
  return rows.split('\n').filter(Boolean).map((row) => {
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

function attempt(what, run) {
  try {
    return run();
  } catch (error) {
    return `(${what} failed: ${error.message})`;
  }
}

function describe(rows) {
  return rows.map((p) => `${String(p.pid).padStart(6)} ${String(p.ppid).padStart(6)} ${p.etime.padStart(11)}  ${p.command}`).join('\n') || '(none)';
}

async function stopTheRun(latest) {
  const seconds = Math.round((Date.now() - latest.at) / 1000);
  const all = processes();
  const run = all.find((p) => p.pid === process.pid);
  const descendants = descendantsOf(process.pid, all);
  fs.mkdirSync(path.dirname(report), { recursive: true });
  const screenshot = report.replace(/\.txt$/, '') + '.png';
  const sections = [
    `# No progress for ${seconds} s since ${latest.what}`,
    `## The latest activity, oldest first\n${recent.map((a) => `${new Date(a.at).toISOString()}  ${a.what}`).join('\n')}`,
    `## The run, under its ancestors (pid, ppid, elapsed, command)\n${describe([...ancestorsOf(process.pid, all), ...(run ? [run] : [])])}`,
    `## What the run started, still running\n${describe(descendants)}`,
    // The main thread's native stack shows whether it's blocked in a synchronous spawn (node::SyncProcessRunner).
    `## The run's threads (sample, 3 s)\n${attempt('sample', () => {
      const file = `${report}.sample`;
      execFileSync('sample', [String(process.pid), '3', '-file', file], { stdio: 'ignore', timeout: 60_000 });
      const text = fs.readFileSync(file, 'utf8');
      fs.rmSync(file);
      return text.split('\nBinary Images:')[0];
    })}`,
    `## Screenshot\n${attempt('screencapture', () => {
      execFileSync('screencapture', ['-x', screenshot], { stdio: 'ignore', timeout: 30_000 });
      return screenshot;
    })}`,
    ...[...outputs].map(([label, text]) => `## ${label} (its end)\n${text}`),
  ];
  fs.writeFileSync(report, sections.join('\n\n') + '\n');
  // Straight to the file descriptor: the worker's process.stderr goes through the main thread, which may be blocked.
  fs.writeSync(2, `\nacceptance: no progress for ${seconds} s since ${latest.what}; stopping the run. What was running, and the main thread's stack: ${report}\n`);

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
  for (const pid of pids) attempt('kill', () => process.kill(pid, 'SIGTERM'));
  const deadline = Date.now() + GRACE_MS;
  while (pids.some(alive) && Date.now() < deadline) await sleep(100);
  for (const pid of pids.filter(alive)) attempt('kill', () => process.kill(pid, 'SIGKILL'));
}
