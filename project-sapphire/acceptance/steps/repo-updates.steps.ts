import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';
import { closeWindowSurfaces } from './workspace-hub.steps';

const previous = new WeakMap<SapphireWorld, string>();
function clone(world: SapphireWorld, name: string): string {
  assert.ok(world.profilePath);
  return path.join(world.profilePath, 'repos', name);
}
function head(world: SapphireWorld, name: string): string {
  return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: clone(world, name), encoding: 'utf8' }).trim();
}
async function log(world: SapphireWorld): Promise<string> {
  return fs.readFileSync((await world.bridge.state()).logPath, 'utf8');
}
Given('the parent clone {string} is detached and its current commit is recorded', function (this: SapphireWorld, name: string) {
  execFileSync('git', ['checkout', '--detach'], { cwd: clone(this, name), stdio: 'ignore' });
  previous.set(this, head(this, name));
});
Given('the upstream of parent clone {string} advances without changing its checkout', function (this: SapphireWorld, name: string) {
  const upstream = path.join(this.scenarioDir, `${name}-upstream`);
  const origin = execFileSync('git', ['remote', 'get-url', 'origin'], { cwd: clone(this, name), encoding: 'utf8' }).trim();
  execFileSync('git', ['clone', origin, upstream], { stdio: 'ignore' });
  fs.writeFileSync(path.join(upstream, 'updated.txt'), 'New upstream content\n');
  execFileSync('git', ['add', '.'], { cwd: upstream });
  execFileSync('git', ['-c', 'user.name=test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false', 'commit', '-m', 'advance origin'], { cwd: upstream, stdio: 'ignore' });
  execFileSync('git', ['push', 'origin', 'HEAD:main'], { cwd: upstream, stdio: 'ignore' });
  this.expectedHead = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: upstream, encoding: 'utf8' }).trim();
});
When('the user requests a repo update round from the palette', async function (this: SapphireWorld) {
  await closeWindowSurfaces(this);
  await this.bridge.palette('shiftShift');
  await this.bridge.waitFor('the repo update command', () => this.bridge.state(), state => state.palette?.items.some(item => item.id === 'repo-updates') === true);
  await this.bridge.palette('filter', 'Run Repo Updates');
  await this.bridge.press('Return');
});
Then('the manual repo update request is logged', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the queued manual update', () => log(this), text => text.includes('repo-update manual round requested'));
});
Then('the parent clone {string} is still at its recorded commit', function (this: SapphireWorld, name: string) {
  assert.equal(head(this, name), previous.get(this));
});
Then('the parent clone {string} reaches the new origin commit', async function (this: SapphireWorld, name: string) {
  assert.ok(this.expectedHead);
  await this.bridge.waitFor('the parent clone update', async () => head(this, name), value => value === this.expectedHead);
});
Then('the repo update failure is logged without an error banner', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the logged fetch failure', () => log(this), text => /repo-update .*failed:.*fetch/.test(text));
  assert.deepEqual(await this.bridge.scrapeErrors(), []);
});
Then('the updater config stores interval {int} and enabled false', async function (this: SapphireWorld, minutes: number) {
  const config = await this.bridge.waitFor('the typed updater config', async () => JSON.parse(fs.readFileSync(this.testConfigPath, 'utf8')), value => value['repo-update']?.['interval-minutes'] === minutes && value['repo-update']?.enabled === false);
  assert.equal(config['repo-update']['interval-minutes'], minutes);
  assert.equal(config['repo-update'].enabled, false);
});
