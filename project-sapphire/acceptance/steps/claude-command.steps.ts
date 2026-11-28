import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';

/** Where a scenario's Claude Code commands are installed: beside its config, on nobody's PATH, so each is set by its full path. */
function commandPath(world: SapphireWorld, name: string): string {
  return path.join(path.dirname(world.testConfigPath), 'commands', name);
}

/** The agents each installed command launched: one line per run, the directory it ran in. */
function runsLog(world: SapphireWorld): string {
  return path.join(path.dirname(world.testConfigPath), 'command-runs.log');
}

function install(world: SapphireWorld, name: string, script: string): void {
  const file = commandPath(world, name);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, script, { mode: 0o755 });
}

function runsThrough(world: SapphireWorld, agent: string): boolean {
  const space = path.join('agent-spaces', agent);
  return fs.existsSync(runsLog(world)) && fs.readFileSync(runsLog(world), 'utf8').split('\n').some((directory) => directory.endsWith(space));
}

Given('the profile {string} runs Claude Code through the command {string}', function (this: SapphireWorld, _profile: string, name: string) {
  const file = path.join(this.profilePath!, 'config.json');
  const config = JSON.parse(fs.readFileSync(file, 'utf8'));
  config.claude = { ...config.claude, command: commandPath(this, name) };
  fs.writeFileSync(file, JSON.stringify(config));
});

// A wrapper as a user would write one: it notes its run, then becomes Claude Code (the harness's fake CLI).
Given('the command {string} is installed', function (this: SapphireWorld, name: string) {
  install(this, name, `#!/bin/sh\npwd >> '${runsLog(this)}'\nexec "$DEVORA_TEST_CLAUDE_PROGRAM" "$@"\n`);
});

Given('the command {string} is installed, but is not Claude Code', function (this: SapphireWorld, name: string) {
  install(this, name, '#!/bin/sh\necho gateway 1.0\n');
});

// Closing an agent's tab pauses it, and opening it again launches its CLI anew.
When('the user closes the agent {string} and opens it again', async function (this: SapphireWorld, name: string) {
  const tab = (await this.bridge.state()).tabs.find((tab) => tab.title === name);
  assert.ok(tab, `${name} has a tab`);
  await this.bridge.dispatch({ command: 'closeTab', id: tab.id });
  await this.bridge.waitFor(`${name}'s tab closed`, () => this.bridge.state(), (s) => !s.tabs.some((tab) => tab.title === name));
  if (!(await this.bridge.state()).agentHub?.loaded) await this.bridge.press('s', ['control']);
  await this.bridge.waitFor('the Agent Hub loaded', () => this.bridge.state(), (s) => s.agentHub?.loaded === true);
  await this.bridge.agentHub({ action: 'open', name });
  await this.bridge.waitFor(`${name}'s chat running`, () => this.bridge.agentChat(), (s) => s.phase === 'running');
});

Then('the agent {string} should run through the command', function (this: SapphireWorld, agent: string) {
  assert.ok(runsThrough(this, agent), `the command ran in ${agent}'s space`);
});

Then('the agent {string} should run claude itself', function (this: SapphireWorld, agent: string) {
  assert.ok(!runsThrough(this, agent), `the command never ran in ${agent}'s space`);
});

Then('the agent chat should say its launch failed: {string}', async function (this: SapphireWorld, why: string) {
  const chat = await this.bridge.waitFor(`the failure "${why}"`, () => this.bridge.agentChat(), (s) => s.phase === 'failed' && s.failure?.includes(why) === true);
  assert.ok(chat.failure!.startsWith('The Claude Code command “'), chat.failure!);
  assert.ok(chat.failure!.includes(' for profile Work '), chat.failure!);
});

When("the user retries the agent's launch", async function (this: SapphireWorld) {
  await this.bridge.chatAction('retry');
});

Then('the agent chat should show no failure', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the chat running with no failure line', () => this.bridge.agentChat(), (s) => s.phase === 'running' && s.failure === null);
});
