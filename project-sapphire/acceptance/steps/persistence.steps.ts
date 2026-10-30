import { Given, Then, When } from '@cucumber/cucumber';
import { execFileSync } from 'node:child_process';
import * as assert from 'node:assert/strict';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { launch, quit, stop, waitUntilReady } from '../support/app';
import type { AppState } from '../support/bridge';
import { SapphireWorld, type RestartedTabs } from '../support/world';

const CORRUPT_BYTES = Buffer.alloc(8192, 0x5a);

function tabsOf(state: AppState): RestartedTabs {
  return {
    tabs: state.tabs.map(({ id, title, kind }) => ({ id, title, kind })),
    activeTab: state.activeTab,
  };
}

function stateDb(world: SapphireWorld): string {
  return path.join(world.stateDir, 'state.db');
}

/** Remembers the tabs, stops the app the given way, and relaunches it on the same state. */
async function restart(world: SapphireWorld, stopApp: () => Promise<void>): Promise<void> {
  world.tabsBeforeRestart = tabsOf(await world.bridge.state());
  await stopApp();
  await world.launch();
  await waitUntilReady(world.bridge);
}

When('Sapphire is quit and relaunched', async function (this: SapphireWorld) {
  await restart(this, () => quit({ app: this.app, bridge: this.bridge }));
});

When('Sapphire is killed and relaunched', async function (this: SapphireWorld) {
  await restart(this, () => stop(this.app, 'SIGKILL'));
});

Given('Sapphire has quit', async function (this: SapphireWorld) {
  await quit({ app: this.app, bridge: this.bridge });
});

When('Sapphire is launched', async function (this: SapphireWorld) {
  await this.launch();
});

Then('the same sessions should be open, with the same one active', async function (this: SapphireWorld) {
  // Ready means every restored terminal runs again.
  const restored = await waitUntilReady(this.bridge);
  assert.deepEqual(tabsOf(restored), this.tabsBeforeRestart);
});

Then('the terminal should not contain {string}', async function (this: SapphireWorld, unexpected: string) {
  // Once the shell has printed its prompt, it is the fresh shell's whole output.
  const text = await this.bridge.waitFor('the active terminal to show its prompt', () => this.bridge.terminalText(), (text) => text.trim() !== '');
  assert.ok(!text.includes(unexpected), `the terminal shows "${unexpected}":\n${text}`);
});

Given('Sapphire stopped while a session was creating its workspace', async function (this: SapphireWorld) {
  await quit({ app: this.app, bridge: this.bridge });
  // Sapphire can't create workspaces yet, so the tab is written as a creation in progress would have left it.
  execFileSync('sqlite3', [
    stateDb(this),
    `INSERT INTO tabs (id, position, title, kind, profile_path)
       SELECT next_id + 1, (SELECT count(*) FROM tabs), 'New task', 'creating_workspace', NULL FROM meta;
     INSERT INTO panes (id, tab_id, kind) SELECT next_id + 2, next_id + 1, 'terminal' FROM meta;
     UPDATE meta SET active_tab = next_id + 1, next_id = next_id + 2;`,
  ]);
});

Then('that session should show that its workspace creation was interrupted', async function (this: SapphireWorld) {
  const state = await waitUntilReady(this.bridge);
  const task = state.tabs.find((tab) => tab.title === 'New task');
  assert.deepEqual(
    { kind: task?.kind, terminal: task?.terminal, active: task?.id === state.activeTab },
    { kind: 'creationFailed', terminal: 'none', active: true },
  );
});

Given('the app state is corrupt', function (this: SapphireWorld) {
  // The WAL too: the app exits without checkpointing it, so the state's pages are in it, and SQLite would read them from there.
  // A WAL with nothing valid in it, SQLite deletes on its own.
  for (const file of [stateDb(this), `${stateDb(this)}-wal`]) fs.writeFileSync(file, CORRUPT_BYTES);
});

Then("Sapphire should report that the app state can't be opened", async function (this: SapphireWorld) {
  const state = await this.bridge.waitFor('the startup problem', () => this.bridge.state(), (s) => s.startupProblem !== null);
  assert.equal(state.startupProblem?.kind, 'stateUnreadable');
  assert.equal(state.startupProblem?.path, stateDb(this));
  assert.deepEqual(state.tabs, []);
});

When('the user chooses to start fresh', async function (this: SapphireWorld) {
  await this.bridge.dispatch({ command: 'startFresh' });
  await waitUntilReady(this.bridge);
});

Then('the corrupt app state should be kept beside the new one', function (this: SapphireWorld) {
  const files = fs.readdirSync(this.stateDir);
  const kept = files.filter((name) => /^state-unreadable-\d{8}T\d{6}Z\.db$/.test(name));
  assert.equal(kept.length, 1, `files in the state dir: ${files.join(', ')}`);
  assert.deepEqual(fs.readFileSync(path.join(this.stateDir, kept[0])), CORRUPT_BYTES);
});

When('a second Sapphire is launched on the same app state', async function (this: SapphireWorld) {
  this.second = await launch(this.testConfigPath, this.stateDir);
});

Then('the second Sapphire should report that another Sapphire is using the app state', async function (this: SapphireWorld) {
  const { bridge } = this.second!;
  const state = await bridge.waitFor('the startup problem', () => bridge.state(), (s) => s.startupProblem !== null);
  assert.deepEqual(state.startupProblem, { kind: 'stateInUse', path: stateDb(this) });
  assert.deepEqual(state.tabs, []);
});

When('the second Sapphire quits', async function (this: SapphireWorld) {
  await quit(this.second!);
});
