import { execFileSync, execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { AppDriver } from './app-driver';
import {
  writeToTerminal,
  getTerminalContent,
  writeBytesToTerminal,
  KEY_CR,
  KEY_ARROW_UP,
} from './terminal-helper';

export type ClaudeHookType = 'PostToolUse' | 'PermissionRequest';

export const CLAUDE_CONFIG_PATH = '/tmp/ember-bdd-claude';

/** A native terminal uses the same interactive CLI/onboarding/permission/exit protocol. */
export interface ClaudeTerminal {
  start(): Promise<void>;
  text(): Promise<string>;
  bytes(data: readonly number[]): Promise<void>;
  exitAll(): Promise<void>;
}
type ClaudeDriver = AppDriver | ClaudeTerminal;
const text = (driver: ClaudeDriver) => 'text' in driver ? driver.text() : getTerminalContent(driver);
const sendBytes = (driver: ClaudeDriver, data: readonly number[]) => 'bytes' in driver ? driver.bytes(data) : writeBytesToTerminal(driver, data);


export function createTestWorkspace(): string {
  const workspacePath = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-bdd-'));

  execSync('git init', { cwd: workspacePath, stdio: 'ignore' });
  trustWorkspace(workspacePath);

  fs.writeFileSync(
    path.join(workspacePath, 'main.py'),
    'def hello():\n    return "world"\n',
  );

  return workspacePath;
}

/**
 * Pre-accept Claude Code's folder-trust dialog for `workspacePath`, so the tests don't depend on how that dialog is drawn or which option it defaults.
 */
function trustWorkspace(workspacePath: string): void {
  const running = testClaudeProcesses();
  if (running.length > 0) throw new Error(`Cannot update workspace trust while test Claude processes are running: ${running.join(', ')}`);
  const configFile = path.join(CLAUDE_CONFIG_PATH, '.claude.json');
  const config = fs.existsSync(configFile) ? JSON.parse(fs.readFileSync(configFile, 'utf-8')) : {};
  config.projects ??= {};
  for (const key of new Set([workspacePath, fs.realpathSync(workspacePath)])) {
    config.projects[key] = { ...config.projects[key], hasTrustDialogAccepted: true };
  }
  const temporary = path.join(fs.mkdtempSync(path.join(CLAUDE_CONFIG_PATH, '.trust-')), 'config.json');
  try {
    fs.writeFileSync(temporary, JSON.stringify(config, null, 2), { mode: 0o600 });
    fs.renameSync(temporary, configFile);
  } finally {
    fs.rmSync(path.dirname(temporary), { recursive: true, force: true });
  }
}

export function setupClaudeHook(
  workspacePath: string,
  logFilePath: string,
  hookType: ClaudeHookType,
): void {
  const claudeDir = path.join(workspacePath, '.claude');
  fs.mkdirSync(claudeDir, { recursive: true });

  // The hook opens this file in a preview pane via the IPC server, so a test can see that the hook reached the session's own tab.
  const previewPath = path.join(claudeDir, 'hook-preview.md');
  fs.writeFileSync(previewPath, '# Hook preview\n');

  const hookScriptPath = path.join(claudeDir, 'bdd-test-hook.sh');
  const hookScript = [
    '#!/bin/bash',
    `echo "PTY_ID=$DEVORA_PTY_ID" >> ${JSON.stringify(logFilePath)}`,
    `echo "IPC_PORT=$DEVORA_IPC_PORT" >> ${JSON.stringify(logFilePath)}`,
    '',
    'if [ -n "$DEVORA_IPC_PORT" ] && [ -n "$DEVORA_PTY_ID" ]; then',
    '  curl -s -o /dev/null -X POST "http://127.0.0.1:$DEVORA_IPC_PORT/preview/open" \\',
    '    -H "Content-Type: application/json" \\',
    `    -d "{\\"ptyId\\": $DEVORA_PTY_ID, \\"path\\": \\"${previewPath}\\", \\"stack\\": false}"`,
    'fi',
    '',
  ].join('\n');
  fs.writeFileSync(hookScriptPath, hookScript, { mode: 0o755 });

  const settings = {
    hooks: {
      [hookType]: [
        {
          matcher: '.*',
          hooks: [
            {
              type: 'command',
              command: hookScriptPath,
            },
          ],
        },
      ],
    },
  };
  fs.writeFileSync(
    path.join(claudeDir, 'settings.json'),
    JSON.stringify(settings, null, 2),
  );
}

export function readHookLog(logFilePath: string): Map<string, string> {
  const entries = new Map<string, string>();
  if (!fs.existsSync(logFilePath)) return entries;

  const content = fs.readFileSync(logFilePath, 'utf-8');
  for (const line of content.split('\n')) {
    const eqIndex = line.indexOf('=');
    if (eqIndex === -1) continue;
    entries.set(line.slice(0, eqIndex), line.slice(eqIndex + 1));
  }
  return entries;
}

export function cleanupWorkspace(workspacePath: string): void {
  fs.rmSync(workspacePath, { recursive: true, force: true });
}

const ONBOARDING_STEPS = [
  { id: 'theme', needle: 'Choose the text style', keys: [KEY_CR] },
  { id: 'apikey', needle: 'Do you want to use this API key', keys: [KEY_ARROW_UP, KEY_CR], delayMs: 100 },
  { id: 'security', needle: 'Press Enter to continue', keys: [KEY_CR] },
] as const;

/**
 * Dismiss Claude Code onboarding screens (theme picker, API key
 * confirmation, etc.) by detecting known prompts and sending the
 * appropriate keystrokes.
 *
 * Returns true if an action was taken (so the caller can re-poll).
 */
async function dismissOnboardingIfNeeded(
  driver: ClaudeDriver,
  content: string,
  dismissed: Set<string>,
): Promise<boolean> {
  for (const step of ONBOARDING_STEPS) {
    if (dismissed.has(step.id)) continue;
    if (!content.includes(step.needle)) continue;

    for (let i = 0; i < step.keys.length; i++) {
      if (i > 0 && 'delayMs' in step) {
        await new Promise(r => setTimeout(r, step.delayMs));
      }
      await sendBytes(driver, step.keys[i]);
    }
    dismissed.add(step.id);
    return true;
  }

  return false;
}

/**
 * Start Claude Code interactively by typing `ccc` into the terminal.
 * Handles first-run onboarding screens if they appear, then waits for
 * the Claude Code input prompt (`>`) before returning.
 */
export async function startClaudeCode(driver: ClaudeDriver): Promise<void> {
  if ('start' in driver) await driver.start(); else await writeToTerminal(driver, 'ccc');

  const start = Date.now();
  let lastContent = '';
  const dismissed = new Set<string>();
  while (Date.now() - start < 60_000) {
    const current = await text(driver);
    lastContent = current;
    const lines = current.split('\n');

    // Claude Code's input prompt: a line containing only `❯` (Unicode
    // U+276F) with optional leading/trailing whitespace.
    const hasPrompt = lines.some(l => /^\s*❯\s*$/.test(l));
    if (hasPrompt) {
      if (testClaudeProcesses().length === 0) throw new Error('Claude Code is ready but its test process could not be identified for teardown');
      return;
    }

    await dismissOnboardingIfNeeded(driver, current, dismissed);

    await new Promise(r => setTimeout(r, 200));
  }
  const trimmed = lastContent.split('\n').filter(l => l.trim()).join('\n');
  throw new Error(
    `Claude Code TUI did not start within 60s\n` +
    `--- terminal content (${trimmed.length} chars) ---\n${trimmed}`
  );
}

/**
 * Type a prompt into Claude Code's TUI and submit it.
 *
 * Claude Code runs in raw terminal mode, where CR (0x0d) is the Enter
 * key — not LF (0x0a).  We write the text bytes followed by a bare CR
 * so the TUI recognises the submit action.
 */
export async function sendPromptToClaudeCode(driver: ClaudeDriver, prompt: string): Promise<void> {
  const bytes = Array.from(new TextEncoder().encode(prompt));
  bytes.push(0x0d);
  await sendBytes(driver, bytes);
}

/**
 * Monitor for Claude Code permission dialogs and auto-approve them.
 * Returns a function to stop monitoring.
 *
 * Claude Code in plan mode asks "Do you want to create ...?" before
 * writing files. This function polls the terminal and presses Enter
 * (which selects the default "Yes") whenever such a dialog appears.
 */
export function autoApprovePermissions(driver: ClaudeDriver): () => void {
  let running = true;
  const loop = (async () => {
    while (running) {
      try {
        const content = await text(driver);
        const lines = content.split('\n');
        const hasYesSelected = lines.some(l => l.includes('❯') && l.includes('Yes'));
        if (hasYesSelected && content.includes('Do you want to create')) {
          await sendBytes(driver, KEY_CR);
        }
      } catch {
        // Session or PTY may already be closed
      }
      await new Promise(r => setTimeout(r, 500));
    }
  })();
  // swallow unhandled rejection from the fire-and-forget loop
  loop.catch(() => {});
  return () => { running = false; };
}

/**
 * Start Claude Code, send a prompt, and begin auto-approving permissions.
 * Returns a function to stop the auto-approve loop.
 */
export async function runClaudePrompt(
  driver: ClaudeDriver,
  prompt: string,
): Promise<() => void> {
  await startClaudeCode(driver);
  await sendPromptToClaudeCode(driver, prompt);
  return autoApprovePermissions(driver);
}

/**
 * Cleanly shut down Claude Code by sending the `/exit` command.
 * Uses CR (raw-mode Enter) so the TUI processes it, then verifies every
 * Claude process using this test config has exited before teardown continues.
 */
export async function stopClaudeCode(driver: ClaudeDriver): Promise<void> {
  if (testClaudeProcesses().length === 0) return;
  // A Claude session can be in a background tab. Address every PTY before the
  // generic teardown closes them; writing to just the active tab misses it.
  if ('exitAll' in driver) await driver.exitAll(); else await driver.eval(`
    for (const session of window.__test.sessionManager.getSessions()) {
      const id = session.getPtyId();
      if (id != null) await window.__TAURI_INTERNALS__.invoke('write_pty', {
        id, data: ${JSON.stringify(Array.from(new TextEncoder().encode('/exit\r')))}
      });
    }
  `);
  const deadline = Date.now() + 15_000;
  while (true) {
    const remaining = testClaudeProcesses();
    if (remaining.length === 0) return;
    if (Date.now() >= deadline) throw new Error(`Claude Code did not exit after /exit; remaining test process IDs: ${remaining.join(', ')}`);
    await new Promise(r => setTimeout(r, 100));
  }
}

/** Inspect only candidate CLI processes, returning IDs rather than logging their environments. */
export function testClaudeProcesses(): number[] {
  const processes = execFileSync('ps', ['-axo', 'pid=,comm='], { encoding: 'utf8' });
  const result: number[] = [];
  for (const line of processes.split('\n')) {
    const match = line.trim().match(/^(\d+)\s+(.+)$/);
    if (!match || (!/^(claude(?:-[\w.-]+)?|node)$/.test(path.basename(match[2])) && !match[2].includes('/claude/versions/'))) continue;
    const pid = Number(match[1]);
    let command: string;
    try {
      command = execFileSync('ps', ['eww', '-p', String(pid), '-o', 'command='], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    } catch (error: any) {
      if (error.status === 1) continue; // Exited between the two snapshots.
      throw error;
    }
    if (command.split(/\s+/).includes(`CLAUDE_CONFIG_DIR=${CLAUDE_CONFIG_PATH}`)) result.push(pid);
  }
  return result;
}
