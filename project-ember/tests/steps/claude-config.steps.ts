import assert from 'node:assert';
import * as fs from 'node:fs';
import { DataTable, Given, When, Then } from '@cucumber/cucumber';
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
      const segment = Array.from(need(row, 'the Effort row').querySelectorAll('.segmented-control-btn'))
        .find((b) => b.textContent === ${JSON.stringify(level)});
      need(segment, 'Effort segment ' + ${JSON.stringify(level)}).click();
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
    const off = Array.from(need(row, 'the Default model row').querySelectorAll('.segmented-control-btn'))
      .find((b) => b.textContent === 'Off');
    need(off, 'the Default model Off segment').click();
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
      const custom = Array.from(need(findRow(), 'the Permission mode row').querySelectorAll('.segmented-control-btn'))
        .find((b) => b.textContent === 'Custom');
      need(custom, 'the Permission mode Custom segment').click();
      // The card re-renders synchronously to reveal the chips.
      const chip = Array.from(findRow().querySelectorAll('.config-chip')).find((c) => c.textContent === ${JSON.stringify(mode)});
      need(chip, 'Permission mode chip ' + ${JSON.stringify(mode)}).click();
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

/** The card's row for a Claude setting key, by the label it shows. */
const ROW_LABELS: Record<string, string> = {
  'opus-model': 'Opus tier',
  'sonnet-model': 'Sonnet tier',
  'haiku-model': 'Haiku tier',
  'permission-mode': 'Permission mode',
};

function rowLookup(key: string): string {
  const label = ROW_LABELS[key];
  if (!label) throw new Error(`no Claude row for ${key}`);
  return `Array.from(document.querySelectorAll('.settings-card .config-row'))
    .find((r) => r.querySelector('.config-row-name')?.textContent === ${JSON.stringify(label)})`;
}

When('the user switches Claude {string} to {string}', async function (this: EmberWorld, key: string, segment: string) {
  await this.driver.eval(`
    const row = need(${rowLookup(key)}, 'the ${key} row');
    const button = Array.from(row.querySelectorAll('.segmented-control-btn')).find((b) => b.textContent === ${JSON.stringify(segment)});
    need(button, '${key} segment ${segment}').click();
  `);
  await this.driver.pollFor(
    `return ${rowLookup(key)}?.querySelector('.segmented-control-btn.segmented-control-active')?.textContent ?? null`,
    segment,
    5_000,
  );
});

Then('Claude {string} should suggest the model IDs, in order:', async function (this: EmberWorld, key: string, table: DataTable) {
  const expected = table.raw().map(([id]) => id);
  // The input's own list fills when the core answers, after the row renders.
  await this.driver.pollFor(
    `
    const input = ${rowLookup(key)}?.querySelector('input.config-input');
    const list = input && document.getElementById(input.getAttribute('list') ?? '');
    return JSON.stringify(list ? Array.from(list.options).map((o) => o.value) : null);
    `,
    JSON.stringify(expected),
    30_000,
  );
});
