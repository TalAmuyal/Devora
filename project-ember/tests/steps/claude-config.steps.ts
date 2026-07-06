import assert from 'node:assert';
import * as fs from 'node:fs';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { UIDriver } from '../support/ui-driver';
import { assertDropdownItemsHitTestable } from '../support/dropdown-helper';

// Merges a single `claude.<key>` override into the global test config.
// The Rust backend reads this file fresh on each create_pty, and the After hook resets it, so the override is scoped to the scenario.
// Read-modify-write so several Givens compose.
function setClaudeSetting(world: EmberWorld, key: string, value: string | null): void {
  const path = world.testConfigPath!;
  let config: { claude?: Record<string, unknown> } = {};
  try {
    config = JSON.parse(fs.readFileSync(path, 'utf8'));
  } catch {
    // missing/empty config — start from scratch
  }
  config.claude = { ...(config.claude ?? {}), [key]: value };
  fs.writeFileSync(path, JSON.stringify(config));
}

Given(
  'the global config sets the Claude {string} to {string}',
  function (this: EmberWorld, key: string, value: string) {
    setClaudeSetting(this, key, value);
  },
);

Given(
  'the global config sets the Claude {string} to None',
  function (this: EmberWorld, key: string) {
    setClaudeSetting(this, key, null);
  },
);

// ---------------------------------------------------------------------------
// Claude Models & Effort card (Settings Hub)
// ---------------------------------------------------------------------------

When('the user switches the Effort row to Custom', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  // The card's rows render after its async settings read.
  await ui.waitForElement('.claude-config-card .claude-config-row');
  await this.driver.eval(`
    const rows = Array.from(document.querySelectorAll('.claude-config-card .claude-config-row'));
    const row = rows.find((r) => r.querySelector('.claude-config-row-name')?.textContent === 'Effort');
    if (!row) throw new Error('Effort row not found in the Claude config card');
    const custom = Array.from(row.querySelectorAll('.segmented-control-btn'))
      .find((b) => b.textContent === 'Custom');
    if (!custom) throw new Error('Custom segment not found in the Effort row');
    custom.click();
  `);
  await ui.waitForElement('.claude-config-card .dropdown-trigger', 3_000);
});

When('the user opens the effort dropdown', async function (this: EmberWorld) {
  const ui = new UIDriver(this.driver);
  await ui.click('.claude-config-card .dropdown-trigger');
  await ui.waitForElement('.claude-config-card .dropdown-popup', 3_000);
});

Then(
  'the effort dropdown items should be clickable at their on-screen position',
  async function (this: EmberWorld) {
    await assertDropdownItemsHitTestable(this.driver, '.claude-config-card .dropdown-popup');
  },
);

When(
  'the user chooses the effort level {string}',
  async function (this: EmberWorld, level: string) {
    await this.driver.eval(`
      const options = Array.from(
        document.querySelectorAll('.claude-config-card .dropdown-popup .dropdown-item-option'),
      );
      const option = options.find(
        (o) => o.querySelector('.dropdown-item-label')?.textContent === ${JSON.stringify(level)},
      );
      if (!option) throw new Error('Effort option not found: ' + ${JSON.stringify(level)});
      option.click();
    `);
    // The card persists, re-reads, and re-renders; the trigger label showing the level marks the write as committed.
    await this.driver.pollFor(
      `return document.querySelector('.claude-config-card .dropdown-trigger-label')?.textContent ?? null`,
      level,
      5_000,
    );
  },
);

Then(
  'the global config should have the Claude {string} set to {string}',
  function (this: EmberWorld, key: string, value: string) {
    const config = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf8'));
    assert.strictEqual(
      config.claude?.[key],
      value,
      `Global config should have claude.${key} = ${value}, got: ${JSON.stringify(config.claude)}`,
    );
  },
);
