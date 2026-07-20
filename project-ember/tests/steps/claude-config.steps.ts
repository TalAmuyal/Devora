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
// Claude Launch Settings card (Settings Hub)
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

// The Default model row is a 2-state On/Off toggle; clicking Off imposes None (the ANTHROPIC_MODEL var is omitted).
When('the user turns the default model off', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `
    const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
    return rows.some((r) => r.querySelector('.config-row-name')?.textContent === 'Default model');
    `,
    true,
    5_000,
  );
  await this.driver.eval(`
    const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
    const row = rows.find((r) => r.querySelector('.config-row-name')?.textContent === 'Default model');
    if (!row) throw new Error('Default model row not found in the Claude config card');
    const off = Array.from(row.querySelectorAll('.segmented-control-btn')).find((b) => b.textContent === 'Off');
    if (!off) throw new Error('Off segment not found in the Default model toggle');
    off.click();
  `);
  // The card persists, re-reads, and re-renders; the Off segment becoming active marks the write as committed.
  await this.driver.pollFor(
    `
    const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
    const row = rows.find((r) => r.querySelector('.config-row-name')?.textContent === 'Default model');
    const active = row?.querySelector('.segmented-control-btn.segmented-control-active');
    return active?.textContent ?? null;
    `,
    'Off',
    5_000,
  );
});

// The Permission mode row is a tri-state combo; switching to Custom reveals suggestion chips, and clicking one commits it.
When(
  'the user sets the permission mode to {string}',
  async function (this: EmberWorld, mode: string) {
    await this.driver.pollFor(
      `
      const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
      return rows.some((r) => r.querySelector('.config-row-name')?.textContent === 'Permission mode');
      `,
      true,
      5_000,
    );
    await this.driver.eval(`
      const findRow = () => Array.from(document.querySelectorAll('.settings-card .config-row'))
        .find((r) => r.querySelector('.config-row-name')?.textContent === 'Permission mode');
      let row = findRow();
      if (!row) throw new Error('Permission mode row not found in the Claude config card');
      const custom = Array.from(row.querySelectorAll('.segmented-control-btn')).find((b) => b.textContent === 'Custom');
      if (!custom) throw new Error('Custom segment not found in the Permission mode combo');
      custom.click();
      row = findRow(); // the card re-renders synchronously to reveal the chips
      const chip = Array.from(row.querySelectorAll('.config-chip')).find((c) => c.textContent === ${JSON.stringify(mode)});
      if (!chip) throw new Error('Permission mode chip not found: ' + ${JSON.stringify(mode)});
      chip.click();
    `);
    // The commit persists, re-reads, and re-renders; the value appearing in the Custom input marks the write as committed.
    await this.driver.pollFor(
      `
      const rows = Array.from(document.querySelectorAll('.settings-card .config-row'));
      const row = rows.find((r) => r.querySelector('.config-row-name')?.textContent === 'Permission mode');
      return row?.querySelector('.config-input')?.value ?? null;
      `,
      mode,
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

Then(
  'the global config should have the Claude {string} set to None',
  function (this: EmberWorld, key: string) {
    const config = JSON.parse(fs.readFileSync(this.testConfigPath!, 'utf8'));
    assert.strictEqual(
      config.claude?.[key],
      null,
      `Global config should have claude.${key} = null (None), got: ${JSON.stringify(config.claude)}`,
    );
  },
);
