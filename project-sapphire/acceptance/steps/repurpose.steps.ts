import assert from 'node:assert';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';
import { openWorkspaceHub } from './workspace-hub.steps';

function workspace(world: SapphireWorld, id: string): string {
  return path.join(world.fixtureRoot!, 'Work', 'workspaces', id);
}
function task(world: SapphireWorld, id: string): { uid: string; title: string } {
  return JSON.parse(fs.readFileSync(path.join(workspace(world, id), 'task.json'), 'utf8'));
}
async function eventually(check: () => boolean): Promise<void> {
  const end = Date.now() + 10_000;
  while (!check()) {
    if (Date.now() >= end) throw new Error('Timed out waiting for task.json');
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

Given('a session is opened for workspace {string} from the Hub', async function (this: SapphireWorld, id: string) {
  await openWorkspaceHub(this);
  await this.bridge.dispatch({ command: 'openWorkspace', id });
  await closeHub(this);
  await this.bridge.waitFor('the session to open', () => this.bridge.state(), (s) => s.activeTab !== null);
});
async function closeHub(world: SapphireWorld): Promise<void> { await world.bridge.clearWindowSurfaces(); }

When('the user runs the {string} palette command', async function (this: SapphireWorld, title: string) {
  await this.bridge.clearWindowSurfaces();
  await this.bridge.palette('shiftShift');
  await this.bridge.waitFor('the palette to open', () => this.bridge.state(), (s) => s.palette !== null);
  await this.bridge.palette('filter', title);
  await this.bridge.press('Return');
});
Then('the repurpose dialog should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the repurpose dialog', () => this.bridge.state(), (s) => s.repurpose !== null);
});
Then('no repurpose dialog should appear', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the error to be shown', () => this.bridge.state(), (s) => s.errorBanners.length > 0);
  assert.strictEqual((await this.bridge.state()).repurpose, null);
});
Then('the repurpose dialog input should be pre-filled with {string}', async function (this: SapphireWorld, title: string) {
  assert.strictEqual((await this.bridge.state()).repurpose?.title, title);
});
When('the user replaces the repurpose dialog input with {string}', async function (this: SapphireWorld, title: string) { await this.bridge.repurpose('title', title); });
When('the user confirms the repurpose dialog', async function (this: SapphireWorld) { await this.bridge.repurpose('confirm'); });
When('the user cancels the repurpose dialog', async function (this: SapphireWorld) { await this.bridge.repurpose('cancel'); });
Given('the current task uid of workspace {string} is noted', function (this: SapphireWorld, id: string) {
  this.originalTaskUid = task(this, id).uid;
  assert.ok(this.originalTaskUid);
});
Then('the task uid of workspace {string} should have changed', async function (this: SapphireWorld, id: string) {
  await eventually(() => task(this, id).uid !== this.originalTaskUid);
});
Then('the task uid of workspace {string} should be unchanged', function (this: SapphireWorld, id: string) {
  assert.strictEqual(task(this, id).uid, this.originalTaskUid);
});
Then('the task.json title of workspace {string} should be {string}', async function (this: SapphireWorld, id: string, title: string) {
  await eventually(() => task(this, id).title === title);
});
Then('the active session title should be {string}', async function (this: SapphireWorld, title: string) {
  await this.bridge.waitFor('the active tab title', () => this.bridge.state(), (s) => s.tabs.find((t) => t.id === s.activeTab)?.title === title);
});
Then('the first session title should be {string}', async function (this: SapphireWorld, title: string) {
  await this.bridge.waitFor('the first tab title', () => this.bridge.state(), (s) => s.tabs[0]?.title === title);
});
Given('repo {string} of workspace {string} is on branch {string}', function (this: SapphireWorld, repo: string, id: string, branch: string) {
  execFileSync('git', ['checkout', '-b', branch], { cwd: path.join(workspace(this, id), repo), stdio: 'ignore' });
});
