import assert from 'node:assert';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { assertActivePanelOverlay } from '../support/overlay-helper';
import {
  createTestWorkspace,
  setupClaudeHook,
  readHookLog,
  runClaudePrompt,
  ClaudeHookType,
} from '../support/claude-helper';

async function pollForHookLog(logPath: string, timeoutMs: number): Promise<Map<string, string>> {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    const entries = readHookLog(logPath);
    if (entries.size > 0) return entries;
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error(`Hook log not written within ${timeoutMs}ms at ${logPath}`);
}

Given(
  'a workspace session exists with a Claude Code hook',
  async function (this: EmberWorld) {
    const workspacePath = createTestWorkspace();
    const hookLogPath = path.join(workspacePath, '.claude', 'hook-log.txt');
    this.workspacePath = workspacePath;
    this.hookLogPath = hookLogPath;
    setupClaudeHook(workspacePath, hookLogPath, 'PostToolUse');

    await this.driver.eval(
      `await window.__test.sessionManager.createSession('claude-test', ${JSON.stringify(workspacePath)})`,
    );
  },
);

When(
  'Claude Code runs and triggers a tool use',
  { timeout: 120_000 },
  async function (this: EmberWorld) {
    this.stopAutoApprove = await runClaudePrompt(this.driver, 'Write the word hello to test-output.txt');
  },
);

Then(
  'a panel overlay should appear on the active session',
  { timeout: 120_000 },
  async function (this: EmberWorld) {
    await assertActivePanelOverlay(this.driver, true, 90_000);
  },
);

Then(
  'the hook script should have received DEVORA_PTY_ID',
  { timeout: 30_000 },
  async function (this: EmberWorld) {
    const entries = await pollForHookLog(this.hookLogPath!, 15_000);
    const ptyId = entries.get('PTY_ID');
    if (ptyId === undefined) throw new Error('PTY_ID not found in hook log');
    assert.notStrictEqual(ptyId.trim(), '', 'PTY_ID is empty');
    assert.match(ptyId, /^\d+$/, 'PTY_ID is not a number');
  },
);

Then(
  'the hook script should have received DEVORA_IPC_PORT',
  async function (this: EmberWorld) {
    const entries = readHookLog(this.hookLogPath!);
    const ipcPort = entries.get('IPC_PORT');
    if (ipcPort === undefined) throw new Error('IPC_PORT not found in hook log');
    assert.notStrictEqual(ipcPort.trim(), '', 'IPC_PORT is empty');
    assert.match(ipcPort, /^\d+$/, 'IPC_PORT is not a number');
  },
);

const ORDINALS: Record<string, number> = { first: 0, second: 1, third: 2 };

function ordinalToIndex(ordinal: string): number {
  const index = ORDINALS[ordinal];
  if (index === undefined) throw new Error(`Unknown ordinal: ${ordinal}`);
  return index;
}

Given(
  'the {word} session has a workspace with a {word} crit hook',
  async function (this: EmberWorld, ordinal: string, hookType: string) {
    const targetIndex = ordinalToIndex(ordinal);

    const workspacePath = createTestWorkspace();
    const hookLogPath = path.join(workspacePath, '.claude', 'hook-log.txt');
    this.workspacePath = workspacePath;
    this.hookLogPath = hookLogPath;
    setupClaudeHook(workspacePath, hookLogPath, hookType as ClaudeHookType);

    const oldSessionId = await this.driver.eval(
      `return window.__test.sessionManager.getSessions()[${targetIndex}]?.id`,
    );
    if (oldSessionId === undefined) {
      throw new Error(`No session at index ${targetIndex} to replace`);
    }
    await this.driver.eval(
      `window.__test.sessionManager.closeSession(${oldSessionId})`,
    );

    await this.driver.eval(
      `await window.__test.sessionManager.createSession('claude-test', ${JSON.stringify(workspacePath)})`,
    );

    const lastIndex: number = await this.driver.eval(
      'return window.__test.sessionManager.getSessionCount() - 1',
    );
    for (let i = lastIndex; i > targetIndex; i--) {
      await this.driver.eval(
        'window.__test.sessionManager.moveTabBackward()',
      );
    }
  },
);

When(
  'Claude Code triggers a tool use in the {word} session',
  { timeout: 120_000 },
  async function (this: EmberWorld, ordinal: string) {
    const index = ordinalToIndex(ordinal);

    const sessionId = await this.driver.eval(
      `return window.__test.sessionManager.getSessions()[${index}]?.id`,
    );
    if (sessionId === undefined) {
      throw new Error(`No session at index ${index}`);
    }

    const previousSessionId = await this.driver.eval(
      'return window.__test.sessionManager.getActiveSessionId()',
    );

    await this.driver.eval(
      `window.__test.sessionManager.activateSession(${sessionId})`,
    );

    this.stopAutoApprove = await runClaudePrompt(this.driver, 'Write the word hello to test-output.txt');

    // Wait for the PostToolUse hook to fire (confirmed by hook log appearing)
    // before switching back to the previous session. This ensures the crit/open
    // IPC call has been made from the hook script.
    if (this.hookLogPath) {
      await pollForHookLog(this.hookLogPath, 90_000);
    }

    if (previousSessionId !== null && previousSessionId !== sessionId) {
      await this.driver.eval(
        `window.__test.sessionManager.activateSession(${previousSessionId})`,
      );
    }
  },
);

