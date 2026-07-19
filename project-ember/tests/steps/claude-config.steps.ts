import assert from 'node:assert';
import * as fs from 'node:fs';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';

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

// The Effort row is a single segmented control listing every level; clicking a level commits it.
// The `Effort` row name is unique across the detail's cards, so it identifies the Claude card without a card-specific selector.
When(
  'the user sets the effort level to {string}',
  async function (this: EmberWorld, level: string) {
    // The card's rows render after its async settings read; wait for the Effort row specifically.
    await this.driver.pollFor(
      `
      const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
      return rows.some((r) => r.querySelector('.config-row-name')?.textContent === 'Effort');
      `,
      true,
      5_000,
    );
    await this.driver.eval(`
      const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
      const row = rows.find((r) => r.querySelector('.config-row-name')?.textContent === 'Effort');
      if (!row) throw new Error('Effort row not found in the Claude config card');
      const segment = Array.from(row.querySelectorAll('.segmented-control-btn'))
        .find((b) => b.textContent === ${JSON.stringify(level)});
      if (!segment) throw new Error('Effort segment not found: ' + ${JSON.stringify(level)});
      segment.click();
    `);
    // The card persists, re-reads, and re-renders; the level's segment becoming active marks the write as committed.
    await this.driver.pollFor(
      `
      const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
      const row = rows.find((r) => r.querySelector('.config-row-name')?.textContent === 'Effort');
      const active = row?.querySelector('.segmented-control-btn.segmented-control-active');
      return active?.textContent ?? null;
      `,
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
