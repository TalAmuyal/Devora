import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import type { AgentHubState, AppState, NewAgentState } from '../support/bridge';
import { SapphireWorld } from '../support/world';
import { closeWindowSurfaces } from './workspace-hub.steps';

// The Agent Hub is Sapphire's (ADR-012). It opens with Ctrl+S, as a user opens it, and is clicked through the bridge's `agent-hub` and `new-agent` routes.

const AGENT_HUB = 'agent-hub';

async function waitForAgentHub(world: SapphireWorld, what: string, done: (hub: AgentHubState) => boolean): Promise<AgentHubState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.agentHub !== null && done(s.agentHub));
  return state.agentHub!;
}

async function waitForDialog(world: SapphireWorld, what: string, done: (dialog: NewAgentState) => boolean): Promise<NewAgentState> {
  const state = await world.bridge.waitFor(what, () => world.bridge.state(), (s) => s.newAgent !== null && done(s.newAgent));
  return state.newAgent!;
}

function field(dialog: NewAgentState, key: string) {
  return dialog.fields.find((f) => f.key === key);
}

function agentSpace(world: SapphireWorld, name: string): string {
  return path.join(world.profilePath!, 'agent-spaces', name);
}

async function openAgentHub(world: SapphireWorld): Promise<void> {
  if ((await world.bridge.state()).windowLayers.length > 0) await closeWindowSurfaces(world);
  await world.bridge.press('s', ['control']);
  await waitForAgentHub(world, 'the Agent Hub to load', (hub) => hub.loaded);
}

async function createAgent(world: SapphireWorld, name: string): Promise<void> {
  await world.bridge.agentHub({ action: 'newAgent' });
  await waitForDialog(world, 'the template fields', (dialog) => dialog.fields.length > 0);
  await world.bridge.newAgent({ action: 'setName', value: name });
  await waitForDialog(world, 'Create to be enabled', (dialog) => dialog.canCreate);
  await world.bridge.newAgent({ action: 'create' });
}

Given('the Agent Hub is open', async function (this: SapphireWorld) {
  await openAgentHub(this);
});

Given('the profile\'s agent template is {string}', function (this: SapphireWorld, template: string) {
  const file = path.join(this.profilePath!, 'agent-template.md');
  fs.writeFileSync(file, template);
  const configPath = path.join(this.profilePath!, 'config.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  config.agent = { ...config.agent, 'template-file': 'agent-template.md' };
  fs.writeFileSync(configPath, JSON.stringify(config));
});

Given('an agent {string} was created', async function (this: SapphireWorld, name: string) {
  await createAgent(this, name);
  await this.bridge.waitFor(`the agent ${name} to open`, () => this.bridge.state(), (s) => s.tabs.some((tab) => tab.kind === 'agentSpace' && tab.title === name));
  await openAgentHub(this);
  await waitForAgentHub(this, `${name} to be listed`, (hub) => hub.items.some((item) => item.name === name));
});

When('the user opens the New Agent dialog', async function (this: SapphireWorld) {
  await this.bridge.agentHub({ action: 'newAgent' });
  await waitForDialog(this, 'the template fields', (dialog) => dialog.fields.length > 0);
});

When('the user names the new agent {string}', async function (this: SapphireWorld, name: string) {
  await this.bridge.newAgent({ action: 'setName', value: name });
  await waitForDialog(this, 'the derived name', (dialog) => field(dialog, 'agent.name')?.value === name);
});

When('the user selects the repo {string} for the new agent', async function (this: SapphireWorld, repo: string) {
  await this.bridge.newAgent({ action: 'toggleRepo', name: repo });
});

When('the user fills the agent field {string} with {string}', async function (this: SapphireWorld, key: string, value: string) {
  await this.bridge.newAgent({ action: 'setField', key, value });
});

When('the user saves the agent field {string}', async function (this: SapphireWorld, key: string) {
  await this.bridge.newAgent({ action: 'save', key });
  await waitForDialog(this, `${key} to be saved`, (dialog) => field(dialog, key)?.saved === true);
});

When('the user creates the agent', async function (this: SapphireWorld) {
  await waitForDialog(this, 'Create to be enabled', (dialog) => dialog.canCreate);
  await this.bridge.newAgent({ action: 'create' });
});

When('the user deletes the agent {string}', async function (this: SapphireWorld, name: string) {
  await this.bridge.agentHub({ action: 'delete', name });
});

Then('the Agent Hub should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the Agent Hub to open', () => this.bridge.state(), (s) => s.windowLayers.includes(AGENT_HUB));
});

Then('the Agent Hub should not be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the Agent Hub to close', () => this.bridge.state(), (s) => !s.windowLayers.includes(AGENT_HUB));
});

Then('the New Agent dialog should show the field {string} from {string}', async function (this: SapphireWorld, key: string, source: string) {
  const dialog = await waitForDialog(this, `the field ${key}`, (d) => field(d, key) !== undefined);
  assert.strictEqual(field(dialog, key)!.source, source.toLowerCase());
});

Then('the New Agent dialog should require the field {string}', async function (this: SapphireWorld, key: string) {
  const dialog = await waitForDialog(this, `the field ${key}`, (d) => field(d, key) !== undefined);
  assert.deepStrictEqual(field(dialog, key), { key, value: '', source: null, saved: false });
  assert.strictEqual(dialog.fields[0].key, key, 'a field only the form fills comes first');
});

Then('the agent can\'t be created yet', async function (this: SapphireWorld) {
  const { newAgent } = await this.bridge.state();
  assert.strictEqual(newAgent?.canCreate, false);
});

Then('the agent can be created', async function (this: SapphireWorld) {
  await waitForDialog(this, 'Create to be enabled', (dialog) => dialog.canCreate);
});

Then('the profile\'s agent config should have the input {string} set to {string}', function (this: SapphireWorld, key: string, value: string) {
  const config = JSON.parse(fs.readFileSync(path.join(this.profilePath!, 'config.json'), 'utf8'));
  assert.strictEqual(config.agent?.inputs?.[key], value);
});

Then('an agent tab {string} should be active', async function (this: SapphireWorld, name: string) {
  await this.bridge.waitFor(`the agent tab ${name}`, () => this.bridge.state(), (s: AppState) => {
    const tab = s.tabs.find((t) => t.id === s.activeTab);
    return tab?.kind === 'agentSpace' && tab.title === name && !s.windowLayers.includes(AGENT_HUB);
  });
});

Then('the agent {string} should be idle', async function (this: SapphireWorld, name: string) {
  await this.bridge.waitFor(`${name} to be idle`, () => this.bridge.state(), (s) => s.tabs.some((tab) => tab.title === name && tab.agentStatus === 'idle'));
});

Then('the agent {string} should have a worktree of {string} and a memory directory', function (this: SapphireWorld, name: string, repo: string) {
  const space = agentSpace(this, name);
  assert.ok(fs.existsSync(path.join(space, repo, '.git')), `${repo} worktree in ${space}`);
  assert.ok(fs.statSync(path.join(space, 'memory')).isDirectory());
  assert.ok(!fs.existsSync(path.join(space, 'CLAUDE.md')), 'nothing is written into the space for Claude to read');
});

Then('the Agent Hub should list no agents', async function (this: SapphireWorld) {
  await waitForAgentHub(this, 'no agents', (hub) => hub.loaded && hub.items.length === 0);
});

Then('the agent space {string} should not exist', function (this: SapphireWorld, name: string) {
  assert.ok(!fs.existsSync(agentSpace(this, name)));
});

When('the user unchecks including coding instructions', async function (this: SapphireWorld) {
  await this.bridge.newAgent({ action: 'setKeepCodingInstructions', value: false });
});

Then('the New Agent preview should contain {string}', async function (this: SapphireWorld, text: string) {
  await waitForDialog(this, `the preview to contain ${text}`, (dialog) => dialog.preview?.text?.includes(text) === true);
});

/** The arguments of the first Claude launch, which the fake CLI reports (`app.ts` puts its report beside the config). */
function firstLaunchArgs(world: SapphireWorld): string[] {
  const report = fs.readFileSync(path.join(path.dirname(world.testConfigPath), 'fake-claude.jsonl'), 'utf8');
  return report.split('\n').filter(Boolean).map((line) => JSON.parse(line)).find((entry) => entry.kind === 'start').args;
}

/** The value after `flag` in `args`, or undefined when the flag is absent. */
function flagValue(args: string[], flag: string): string | undefined {
  const index = args.indexOf(flag);
  return index === -1 ? undefined : args[index + 1];
}

Then('Claude should have been launched with the instructions as its whole system prompt', function (this: SapphireWorld) {
  const args = firstLaunchArgs(this);
  assert.ok(args.includes('--system-prompt'), JSON.stringify(args));
  assert.ok(!args.includes('--append-system-prompt'));
});

Then('the New Agent dialog should start with the model {string} and Claude Code\'s default effort', async function (this: SapphireWorld, model: string) {
  await waitForDialog(this, `model ${model}, default effort`, (dialog) => dialog.model === model && dialog.effort === null && dialog.effortLevels.length > 0);
});

Then('the New Agent dialog should say the space is created at {string} in the profile', async function (this: SapphireWorld, relative: string) {
  await waitForDialog(this, `the space path ending in ${relative}`, (dialog) => dialog.spacePath?.endsWith(`/${relative}`) === true);
});

When('the user sets the new agent\'s model to {string} and its effort to {string}', async function (this: SapphireWorld, model: string, effort: string) {
  await this.bridge.newAgent({ action: 'setModel', value: model });
  await this.bridge.newAgent({ action: 'setEffort', value: effort });
});

Then('Claude should have been launched with the model {string} and the effort {string}', function (this: SapphireWorld, model: string, effort: string) {
  const args = firstLaunchArgs(this);
  assert.equal(flagValue(args, '--model'), model, JSON.stringify(args));
  assert.equal(flagValue(args, '--effort'), effort, JSON.stringify(args));
});

Then('Claude should have been launched with the model {string} and no effort', function (this: SapphireWorld, model: string) {
  const args = firstLaunchArgs(this);
  assert.equal(flagValue(args, '--model'), model, JSON.stringify(args));
  assert.ok(!args.includes('--effort'), JSON.stringify(args));
});
