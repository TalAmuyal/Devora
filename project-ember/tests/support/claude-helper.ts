import { execSync } from 'node:child_process';
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

export function createTestWorkspace(): string {
  const workspacePath = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-bdd-'));

  execSync('git init', { cwd: workspacePath, stdio: 'ignore' });

  fs.writeFileSync(
    path.join(workspacePath, 'main.py'),
    'def hello():\n    return "world"\n',
  );

  return workspacePath;
}

export function setupClaudeHook(
  workspacePath: string,
  logFilePath: string,
  hookType: ClaudeHookType,
): void {
  const claudeDir = path.join(workspacePath, '.claude');
  fs.mkdirSync(claudeDir, { recursive: true });

  const hookScriptPath = path.join(claudeDir, 'bdd-test-hook.sh');
  const hookScript = [
    '#!/bin/bash',
    `echo "PTY_ID=$DEVORA_PTY_ID" >> ${JSON.stringify(logFilePath)}`,
    `echo "IPC_PORT=$DEVORA_IPC_PORT" >> ${JSON.stringify(logFilePath)}`,
    '',
    'if [ -n "$DEVORA_IPC_PORT" ] && [ -n "$DEVORA_PTY_ID" ]; then',
    '  curl -s -X POST "http://127.0.0.1:$DEVORA_IPC_PORT/crit/open" \\',
    '    -H "Content-Type: application/json" \\',
    '    -d "{\\"ptyId\\": $DEVORA_PTY_ID, \\"url\\": \\"http://localhost/bdd-test-hook\\"}" &',
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
  { id: 'trust', needle: 'Yes, I trust this folder', keys: [KEY_CR] },
] as const;

/**
 * Dismiss Claude Code onboarding screens (theme picker, API key
 * confirmation, etc.) by detecting known prompts and sending the
 * appropriate keystrokes.
 *
 * Returns true if an action was taken (so the caller can re-poll).
 */
async function dismissOnboardingIfNeeded(
  driver: AppDriver,
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
      await writeBytesToTerminal(driver, step.keys[i]);
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
export async function startClaudeCode(driver: AppDriver): Promise<void> {
  await writeToTerminal(driver, 'ccc');

  const start = Date.now();
  let lastContent = '';
  const dismissed = new Set<string>();
  while (Date.now() - start < 60_000) {
    const current = await getTerminalContent(driver);
    lastContent = current;
    const lines = current.split('\n');

    // Claude Code's input prompt: a line containing only `❯` (Unicode
    // U+276F) with optional leading/trailing whitespace.
    const hasPrompt = lines.some(l => /^\s*❯\s*$/.test(l));
    if (hasPrompt) {
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
export async function sendPromptToClaudeCode(driver: AppDriver, prompt: string): Promise<void> {
  const bytes = Array.from(new TextEncoder().encode(prompt));
  bytes.push(0x0d);
  await writeBytesToTerminal(driver, bytes);
}

/**
 * Monitor for Claude Code permission dialogs and auto-approve them.
 * Returns a function to stop monitoring.
 *
 * Claude Code in plan mode asks "Do you want to create ...?" before
 * writing files. This function polls the terminal and presses Enter
 * (which selects the default "Yes") whenever such a dialog appears.
 */
export function autoApprovePermissions(driver: AppDriver): () => void {
  let running = true;
  const loop = (async () => {
    while (running) {
      try {
        const content = await getTerminalContent(driver);
        const lines = content.split('\n');
        const hasYesSelected = lines.some(l => l.includes('❯') && l.includes('Yes'));
        if (hasYesSelected && content.includes('Do you want to create')) {
          await writeBytesToTerminal(driver, KEY_CR);
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
  driver: AppDriver,
  prompt: string,
): Promise<() => void> {
  await startClaudeCode(driver);
  await sendPromptToClaudeCode(driver, prompt);
  return autoApprovePermissions(driver);
}

/**
 * Cleanly shut down Claude Code by sending the `/exit` command.
 * Uses CR (raw-mode Enter) so the TUI processes it, then waits for the
 * shell prompt to return.
 */
export async function stopClaudeCode(driver: AppDriver): Promise<void> {
  try {
    await sendPromptToClaudeCode(driver, '/exit');
    await new Promise(r => setTimeout(r, 2_000));
  } catch {
    // Session or PTY may already be closed
  }
}
