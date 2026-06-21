import assert from 'node:assert';
import { When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';

When('the Health Hub finishes loading', async function (this: EmberWorld) {
  // The Health Hub shells out to `debi health --json` through a login shell, which can take a moment; wait until it renders sections (success) or an error state, then fail loudly on error so a broken integration is obvious.
  await this.driver.pollFor(
    `return !!document.querySelector('.health-section') || !!document.querySelector('.health-error')`,
    true,
    25_000,
  );
  const errorText = await this.driver.eval(
    `return document.querySelector('.health-error')?.textContent ?? null`,
  );
  assert.strictEqual(errorText, null, `Health Hub failed to load: ${errorText}`);
});

Then('the Health Hub should be visible', async function (this: EmberWorld) {
  const visible = await this.driver.eval(`return document.querySelector('.health-hub') !== null`);
  assert.strictEqual(visible, true, 'Health Hub should be visible');
});

Then(
  'the Health Hub should show the {string} section',
  async function (this: EmberWorld, title: string) {
    const titles = (await this.driver.eval(
      `return Array.from(document.querySelectorAll('.health-section-title')).map(e => e.textContent)`,
    )) as string[];
    assert.ok(
      titles.includes(title),
      `expected a "${title}" section, got: ${titles.join(', ')}`,
    );
  },
);

Then('the Health Hub should report the config file as found', async function (this: EmberWorld) {
  const message = (await this.driver.eval(`
    const rows = Array.from(document.querySelectorAll('.health-row'));
    const row = rows.find(r => r.querySelector('.health-row-name')?.textContent === 'config');
    return row?.querySelector('.health-row-msg')?.textContent ?? null;
  `)) as string | null;
  assert.strictEqual(message, 'found', 'config row should report the fixture config as found');
});

Then('the Health Hub should not list kitty', async function (this: EmberWorld) {
  const names = (await this.driver.eval(
    `return Array.from(document.querySelectorAll('.health-row-name')).map(e => e.textContent)`,
  )) as string[];
  assert.ok(!names.includes('kitty'), `kitty should not appear, got: ${names.join(', ')}`);
});

Then('the Health Hub should show a version', async function (this: EmberWorld) {
  const version = (await this.driver.eval(
    `return document.querySelector('.health-version-pill')?.textContent ?? ''`,
  )) as string;
  assert.ok(version.length > 0, 'expected a non-empty version pill');
});
