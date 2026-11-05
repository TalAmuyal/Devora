import assert from 'node:assert';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { Given, When, Then } from '@cucumber/cucumber';
import type { Modifier } from '../support/bridge';
import { SapphireWorld } from '../support/world';

// Keys go through the app's key router, as AppKit's own key-downs do (see `Bridge.press`).

const MODIFIERS: Record<string, Modifier> = { Ctrl: 'control', Shift: 'shift', Option: 'option', Cmd: 'command' };
const KEYS: Record<string, string> = { '-': 'Minus', '=': 'Equal' };

/** A chord as the scenarios write it, e.g. `Ctrl+Shift+Right` or `Ctrl+=`. */
function parseChord(chord: string): { key: string; modifiers: Modifier[] } {
  const parts = chord.split(/\+(?=.)/);
  const name = parts.pop()!;
  const modifiers = parts.map((part) => {
    const modifier = MODIFIERS[part];
    if (!modifier) throw new Error(`Unknown modifier ${part} in ${chord}`);
    return modifier;
  });
  return { key: KEYS[name] ?? (name.length === 1 ? name.toLowerCase() : name), modifiers };
}

When(/^the user presses ((?:Ctrl|Shift|Option|Cmd)\+.+)$/, async function (this: SapphireWorld, chord: string) {
  const { key, modifiers } = parseChord(chord);
  await this.bridge.press(key, modifiers);
});

/** A key as Ember's scenarios name it (its `KeyboardEvent.key`), as the bridge presses it. */
function pressable(name: string): { key: string; modifiers: Modifier[] } {
  const named: Record<string, string> = { Enter: 'Return', ArrowDown: 'Down', ArrowUp: 'Up' };
  if (named[name]) return { key: named[name], modifiers: [] };
  if (name === '?') return { key: 'Slash', modifiers: ['shift'] };
  if (/^[A-Z]$/.test(name)) return { key: name.toLowerCase(), modifiers: ['shift'] };
  return { key: name, modifiers: [] };
}

When('the user presses {string}', async function (this: SapphireWorld, name: string) {
  const { key, modifiers } = pressable(name);
  await this.bridge.press(key, modifiers);
});

When('the user presses {string} {int} times', async function (this: SapphireWorld, name: string, count: number) {
  const { key, modifiers } = pressable(name);
  for (let i = 0; i < count; i++) await this.bridge.press(key, modifiers);
});

Then('the terminal font size should be {int}', async function (this: SapphireWorld, size: number) {
  await this.bridge.waitFor(`the font size to be ${size}`, () => this.bridge.state(), (s) => s.fontSize === size);
});

Then('no session should have the keyboard focus', async function (this: SapphireWorld) {
  const { tabs } = await this.bridge.state();
  assert.deepStrictEqual(tabs.filter((tab) => tab.focused).map((tab) => tab.id), []);
});

/** The PTY size a program in the tab's shell sees, as `stty size` reports it. */
async function ptySize(world: SapphireWorld, tab: number): Promise<{ rows: number; cols: number }> {
  const report = path.join(world.scenarioDir, `stty-${tab}-${Date.now()}`);
  await world.bridge.typeLine(`stty size > '${report}'`, tab);
  const text = await world.bridge.waitFor(`tab ${tab}'s stty size`, async () => fs.existsSync(report) ? fs.readFileSync(report, 'utf8').trim() : '', (value) => /^\d+ \d+$/.test(value));
  const [rows, cols] = text.split(' ').map(Number);
  return { rows, cols };
}

Given('every session\'s terminal size is recorded', async function (this: SapphireWorld) {
  const { tabs } = await this.bridge.state();
  this.recordedSizes = await Promise.all(tabs.map((tab) => ptySize(this, tab.id)));
});

// A smaller font fits more columns: every terminal, the background ones included, must be refit to the same, wider size.
Then('every session\'s terminal should be refit to the same, wider size', async function (this: SapphireWorld) {
  const { tabs } = await this.bridge.state();
  const before = this.recordedSizes!;
  const deadline = Date.now() + 10_000;
  let sizes: { rows: number; cols: number }[] = [];
  while (Date.now() < deadline) {
    sizes = await Promise.all(tabs.map((tab) => ptySize(this, tab.id)));
    if (sizes.every((size, i) => size.cols > before[i].cols && size.cols === sizes[0].cols && size.rows === sizes[0].rows)) return;
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  assert.fail(`terminal sizes before ${JSON.stringify(before)}, after ${JSON.stringify(sizes)}`);
});
