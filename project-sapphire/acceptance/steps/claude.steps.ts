import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { createTestWorkspace, setupClaudeHook, readHookLog, runClaudePrompt, type ClaudeHookType } from '../../../project-ember/tests/support/claude-helper';
import { SapphireWorld } from '../support/world';
import { claudeTerminal } from '../support/claude';

async function workspace(world: SapphireWorld, tab: number, hook: ClaudeHookType) {
  world.workspacePath = createTestWorkspace();
  world.hookLogPath = path.join(world.workspacePath, '.claude/hook-log.txt');
  world.claudeTab = tab;
  setupClaudeHook(world.workspacePath, world.hookLogPath, hook);
}
Given('a workspace session exists with a Claude Code hook', async function (this: SapphireWorld) {
  await this.bridge.clearWindowSurfaces();
  await this.bridge.dispatch({ command: 'openShell' });
  const tab = await this.bridge.waitFor('the hook terminal', () => this.bridge.state(), s => s.tabs.length === 1 && s.tabs[0].terminal === 'running');
  await workspace(this, tab.tabs[0].id, 'PostToolUse');
});
Given('the {word} session has a workspace with a {word} Claude Code hook', async function (this: SapphireWorld, ordinal: string, hook: ClaudeHookType) {
  assert.equal(ordinal, 'first');
  await workspace(this, (await this.bridge.state()).tabs[0].id, hook);
  this.hookRelease = path.join(this.workspacePath!, '.claude/release-hook');
  const script = path.join(this.workspacePath!, '.claude/bdd-test-hook.sh');
  const content = fs.readFileSync(script, 'utf8').replace('if [ -n "$DEVORA_IPC_PORT" ]', `while [ ! -f '${this.hookRelease}' ]; do sleep 0.05; done\nif [ -n "$DEVORA_IPC_PORT" ]`);
  fs.writeFileSync(script, content);
});
When('Claude Code runs and triggers a tool use', { timeout: 120_000 }, async function (this: SapphireWorld) {
  this.stopAutoApprove = await runClaudePrompt(claudeTerminal(this), 'Write the word hello to test-output.txt');
});
When('Claude Code triggers a tool use in the {word} session', { timeout: 120_000 }, async function (this: SapphireWorld, ordinal: string) {
  assert.equal(ordinal, 'first');
  const previous = (await this.bridge.state()).activeTab!;
  await this.bridge.dispatch({ command: 'activateTab', id: this.claudeTab! });
  await this.bridge.waitFor('the CLI tab to be active', () => this.bridge.state(), state => state.activeTab === this.claudeTab);
  this.stopAutoApprove = await runClaudePrompt(claudeTerminal(this), 'Write the word hello to test-output.txt');
  await this.bridge.waitFor('the real PostToolUse hook', async () => readHookLog(this.hookLogPath!), entries => entries.has('PTY_ID'), 90_000);
  await this.bridge.dispatch({ command: 'activateTab', id: previous });
  await this.bridge.waitFor('the other tab to be active', () => this.bridge.state(), state => state.activeTab === previous);
  fs.writeFileSync(this.hookRelease!, 'release');
});
Then('the hook should open a preview pane on the {word} session', { timeout: 120_000 }, async function (this: SapphireWorld, ordinal: string) {
  await this.bridge.waitFor('the hook preview on its owning tab', () => this.bridge.state(), state => (ordinal === 'first' ? state.tabs[0] : state.tabs.find(t => t.id === state.activeTab))?.previews.length === 1, 90_000);
});
Then('the hook script should have received DEVORA_PTY_ID', async function (this: SapphireWorld) {
  const entries = await this.bridge.waitFor('hook context', async () => readHookLog(this.hookLogPath!), entries => entries.has('PTY_ID'), 20_000);
  const state = await this.bridge.state();
  assert.equal(entries.get('PTY_ID'), String(state.tabs.find(t => t.id === this.claudeTab)!.terminalID));
});
Then('the hook script should have received DEVORA_IPC_PORT', async function (this: SapphireWorld) {
  assert.match(readHookLog(this.hookLogPath!).get('IPC_PORT') ?? '', /^[1-9][0-9]*$/);
});
