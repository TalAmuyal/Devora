/** Shared recorded CLI protocol; desktop suites on one machine must run sequentially. */
import { BeforeAll, Before, AfterAll } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { FakeClaudeServer } from '../../../project-ember/tests/support/fake-claude-server';
import { CLAUDE_CONFIG_PATH, stopClaudeCode, testClaudeProcesses, cleanupWorkspace, type ClaudeTerminal } from '../../../project-ember/tests/support/claude-helper';
import type { SapphireWorld } from './world';

let server: FakeClaudeServer;
BeforeAll(async function () {
  if (testClaudeProcesses().length) throw new Error('A previous test Claude process is still running; refusing to reset its configuration');
  server = new FakeClaudeServer();
  await server.start();
  fs.rmSync(CLAUDE_CONFIG_PATH, { recursive: true, force: true });
  fs.mkdirSync(path.join(CLAUDE_CONFIG_PATH, 'plans'), { recursive: true });
  fs.writeFileSync(path.join(CLAUDE_CONFIG_PATH, 'settings.json'), JSON.stringify({ permissions: { defaultMode: 'plan' }, theme: 'dark' }));
  fs.writeFileSync(path.join(CLAUDE_CONFIG_PATH, '.claude.json'), JSON.stringify({ hasSeenAutoDefaultNudge: true, hasSeenEffortMediumNudge: true }));
});
Before({ tags: '@real-claude' }, async function (scenario) {
  await server.loadCassette(scenario.pickle.name.replace(/[^a-zA-Z0-9_-]/g, '_'));
});
AfterAll(async function () {
  await server?.stop();
  if (!testClaudeProcesses().length) fs.rmSync(CLAUDE_CONFIG_PATH, { recursive: true, force: true });
});
export function claudeEnvironment(): Record<string, string> {
  return { ANTHROPIC_BASE_URL: `http://127.0.0.1:${server.getPort()}`, ANTHROPIC_API_KEY: 'sk-ant-fake-key-for-bdd-tests',
    CLAUDE_CONFIG_DIR: CLAUDE_CONFIG_PATH, CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: '1' };
}

/** The bridge sends text and real Return/Up events through Ghostty, addressing the owning tab. */
export function claudeTerminal(world: SapphireWorld): ClaudeTerminal {
  return {
    start: () => world.bridge.typeLine(`cd ${quote(world.workspacePath!)} && ${quote(path.resolve(import.meta.dirname, '../../build/test-tools/ccc'))}`, world.claudeTab),
    text: () => world.bridge.terminalText(world.claudeTab),
    bytes: async data => {
      if (data.join(',') === '27,91,65') { await world.bridge.press('Up'); return; }
      if (data.at(-1) !== 13) throw new Error('Unsupported native CLI input');
      await world.bridge.typeLine(new TextDecoder().decode(new Uint8Array(data.slice(0, -1))), world.claudeTab);
    },
    exitAll: async () => {
      for (const tab of (await world.bridge.state()).tabs) await world.bridge.typeLine('/exit', tab.id);
    },
  };
}
export async function stopTestClaude(world: SapphireWorld): Promise<void> {
  if (world.claudeTab === undefined) return;
  if (world.hookRelease) fs.writeFileSync(world.hookRelease, 'release');
  world.stopAutoApprove?.();
  await stopClaudeCode(claudeTerminal(world));
  server.reset();
  if (world.workspacePath) cleanupWorkspace(world.workspacePath);
}
const quote = (value: string) => "'" + value.replaceAll("'", "'\"'\"'") + "'";
