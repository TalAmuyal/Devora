import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { execFileSync } from 'node:child_process';
import { Given, When, Then, After } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { createTestFixtureRoot, createTestProfile, createTestRepo, writeTestConfig } from '../support/fixture-helper';
import { GIT_TEST_IDENTITY } from '../support/git-test-env';
import { runPaletteCommand } from '../support/command-palette-helper';
import { UIDriver } from '../support/ui-driver';
import { restartApp } from '../support/hooks';

function git(cwd: string, ...args: string[]): string {
  return execFileSync('git', args, { cwd, env: { ...process.env, ...GIT_TEST_IDENTITY }, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}

async function openSettings(world: EmberWorld): Promise<void> {
  await world.driver.eval('window.__test.layers.clear(); window.__test.openSettingsHub();');
  await new UIDriver(world.driver).click('.pm-user-defaults-row');
  await world.driver.eval(`need(Array.from(document.querySelectorAll('.config-row')).find(row => row.querySelector('.config-row-name')?.textContent === 'Periodic updates'), 'Repository Updates card');`);
}

async function segment(world: EmberWorld, name: string, label: string): Promise<void> {
  await world.driver.eval(`
    const row = need(Array.from(document.querySelectorAll('.config-row')).find(row => row.querySelector('.config-row-name')?.textContent === ${JSON.stringify(name)}), 'setting row');
    need(Array.from(row.querySelectorAll('.segmented-control-btn')).find(button => button.textContent === ${JSON.stringify(label)}), 'setting segment').click();
  `);
  await world.driver.pollFor(`return window.__test.invokesInFlight()`, 0, 5_000);
}

When('Ember User Defaults repository settings are opened', async function (this: EmberWorld) { await openSettings(this); });
Then('Ember periodic repo updates default to off', async function (this: EmberWorld) {
  await this.driver.pollFor(`
    const row = Array.from(document.querySelectorAll('.config-row')).find(row => row.querySelector('.config-row-name')?.textContent === 'Periodic updates');
    return row?.querySelector('.config-hint')?.textContent;
  `, '→ off', 5_000);
});
When('Ember periodic repo updates are set to {string}', async function (this: EmberWorld, value: string) { await segment(this, 'Periodic updates', value); });
When('Ember repo update interval is set to {int} minutes', async function (this: EmberWorld, minutes: number) {
  await segment(this, 'Interval (minutes)', 'Set');
  await this.driver.eval(`
    const row = need(Array.from(document.querySelectorAll('.config-row')).find(row => row.querySelector('.config-row-name')?.textContent === 'Interval (minutes)'), 'interval row');
    const input = need(row.querySelector('input'), 'interval input');
    input.focus(); input.value = ${JSON.stringify(String(minutes))};
    input.dispatchEvent(new Event('input', { bubbles: true }));
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  `);
  await this.driver.pollFor('return window.__test.invokesInFlight()', 0, 5_000);
});
Then('Ember updater config stores enabled true and interval {int}', function (this: EmberWorld, minutes: number) {
  const settings = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf8'))['repo-update'];
  assert.strictEqual(settings.enabled, true);
  assert.strictEqual(settings['interval-minutes'], minutes);
});

Given('an Ember parent clone with an advanced origin', function (this: EmberWorld) {
  this.fixtureRoot = createTestFixtureRoot();
  this.profilePath = createTestProfile(this.fixtureRoot, 'Work');
  const repo = createTestRepo(this.profilePath, 'api');
  git(repo, 'branch', '-M', 'main');
  const origin = path.join(this.fixtureRoot, 'origin.git');
  git(this.fixtureRoot, 'clone', '--bare', repo, origin);
  git(repo, 'remote', 'add', 'origin', origin);
  git(repo, 'fetch', 'origin');
  git(repo, 'remote', 'set-head', 'origin', '-a');
  git(repo, 'checkout', '--detach');
  const oldHead = git(repo, 'rev-parse', 'HEAD');
  const upstream = path.join(this.fixtureRoot, 'upstream');
  git(this.fixtureRoot, 'clone', origin, upstream);
  git(upstream, 'commit', '--allow-empty', '-m', 'advance');
  git(upstream, 'push', 'origin', 'main');
  this.repoUpdate = { repo, oldHead, newHead: git(upstream, 'rev-parse', 'HEAD') };
  writeTestConfig(this.testConfigPath!, [this.profilePath]);
});
When('the Ember palette runs {string}', async function (this: EmberWorld, title: string) {
  await this.driver.eval('window.__test.layers.clear();');
  await runPaletteCommand(this.driver, title);
  await this.driver.pollFor('return window.__test.invokesInFlight()', 0, 5_000);
});
Then('the Ember Paused hint is visible', async function (this: EmberWorld) {
  await this.driver.pollFor("return document.querySelector('.global-pause-hint')?.textContent", 'Paused', 5_000);
  assert.strictEqual(JSON.parse(fs.readFileSync(path.join(path.dirname(this.testConfigPath!), 'state/global-paused.json'), 'utf8')), true);
});
Then('the Ember Paused hint is hidden', async function (this: EmberWorld) {
  await this.driver.pollFor("return document.querySelector('.global-pause-hint') === null", true, 5_000);
});
Then('the Ember parent clone keeps its original commit', function (this: EmberWorld) {
  assert.strictEqual(git(this.repoUpdate!.repo, 'rev-parse', 'HEAD'), this.repoUpdate!.oldHead);
});
Then('the Ember parent clone reaches its advanced origin', async function (this: EmberWorld) {
  const deadline = Date.now() + 10_000;
  while (git(this.repoUpdate!.repo, 'rev-parse', 'HEAD') !== this.repoUpdate!.newHead) {
    if (Date.now() > deadline) assert.fail('parent clone did not reach origin');
    await new Promise(resolve => setTimeout(resolve, 50));
  }
});
When('Ember restarts with its saved state', async function (this: EmberWorld) { this.driver = await restartApp(); });

After({ tags: '@repo-updates' }, async function (this: EmberWorld) {
  await this.driver.eval('window.__test.layers.clear();');
  if (await this.driver.eval("return document.querySelector('.global-pause-hint') !== null")) {
    await runPaletteCommand(this.driver, 'Resume Devora');
  }
  await openSettings(this);
  await segment(this, 'Periodic updates', 'Off');
});
