import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { SapphireWorld } from '../support/world';

// What a program started in a new Sapphire terminal sees, at every moment a terminal can be made: libghostty makes a
// terminal created while the app's color scheme differs from the config's default from a replayed copy of the config,
// which drops the environment and command Sapphire gives it (`Sources/TerminalKit/CLAUDE.md`).

const standin = path.resolve(import.meta.dirname, '../fixtures/default-app-standin.sh');

function updateJson(file: string, update: (config: any) => void): void {
  const config = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf-8')) : {};
  update(config);
  fs.writeFileSync(file, JSON.stringify(config, null, 2));
}

Given('the global config sets Appearance to {string}', function (this: SapphireWorld, appearance: string) {
  updateJson(this.testConfigPath, (config) => { config.appearance = appearance; });
});

Given('the profile {string} runs the stand-in as its default app', function (this: SapphireWorld, name: string) {
  updateJson(path.join(this.fixtureRoot!, name, 'config.json'), (config) => {
    config.terminal = { ...(config.terminal ?? {}), 'default-app': standin };
  });
});

When("the user opens the profile's first workspace", async function (this: SapphireWorld) {
  const state = await this.bridge.waitFor('the workspaces', () => this.bridge.state(), (s) => s.workspaces.length > 0);
  const before = new Set(state.tabs.map((tab) => tab.id));
  (this as any).openedWorkspace = state.workspaces[0].path;
  await this.bridge.dispatch({ command: 'openWorkspace', id: state.workspaces[0].id });
  await this.bridge.waitFor('the workspace tab and its terminal', () => this.bridge.state(), (s) =>
    s.activeTab !== null && !before.has(s.activeTab) && s.tabs.some((tab) => tab.id === s.activeTab && tab.terminal === 'running'));
});

Then("the default app should see Devora's environment and start in its workspace", async function (this: SapphireWorld) {
  const workspace = fs.realpathSync((this as any).openedWorkspace as string);
  const text = await this.bridge.waitFor('the stand-in default app to report', () => this.bridge.terminalText(), (t) => /STANDIN pty=\S* port=\S* cwd=\S+/.test(t));
  const [, pty, port, cwd] = text.match(/STANDIN pty=(\S*) port=(\S*) cwd=(\S+)/)!;
  assert.match(pty, /^\d+$/, `DEVORA_PTY_ID in the terminal: "${pty}"`);
  assert.match(port, /^\d+$/, `DEVORA_IPC_PORT in the terminal: "${port}"`);
  assert.ok(fs.realpathSync(cwd).startsWith(workspace), `started in ${cwd}, not in its workspace ${workspace}`);
});

// A program that asks for the color scheme and its changes (DSR 996, mode 2031) prints each report with the background
// an OSC 11 query answered after it (`acceptance/fixtures/scheme-listener.py`).
When('a program listening for color-scheme changes runs in the terminal', async function (this: SapphireWorld) {
  const listener = path.resolve(import.meta.dirname, '../fixtures/scheme-listener.py');
  await this.bridge.waitFor('the active tab to have its terminal', () => this.bridge.terminalText(), () => true);
  await this.bridge.typeLine(`python3 '${listener}'`);
  await this.bridge.waitFor('the listener to start', () => this.bridge.terminalText(), (text) => text.includes('LISTENING'));
});

Then('the program should last have been told {string}, with a {string} background', async function (this: SapphireWorld, scheme: string, shade: string) {
  const last = (text: string) => [...text.matchAll(/SCHEME (\w+) BG rgb:([0-9a-f]+)\/([0-9a-f]+)\/([0-9a-f]+)/gi)].at(-1);
  await this.bridge.waitFor(`a "${scheme}" report on a ${shade} background`, () => this.bridge.terminalText(), (text) => {
    const report = last(text);
    if (!report || report[1] !== scheme) return false;
    // Each channel is a hex fraction of its full scale (rgb:rrrr/gggg/bbbb, or fewer digits).
    const brightness = [report[2], report[3], report[4]].map((v) => parseInt(v, 16) / (16 ** v.length - 1));
    const light = brightness.reduce((a, b) => a + b) / 3 > 0.5;
    return light === (shade === 'light');
  });
});
