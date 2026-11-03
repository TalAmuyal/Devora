import assert from 'node:assert';
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
