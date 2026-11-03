import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';
import { closeWindowSurfaces } from './workspace-hub.steps';

Given('no overlay is open', async function (this: SapphireWorld) { await closeWindowSurfaces(this); });

Given('the Command Palette is open', async function (this: SapphireWorld) {
  await closeWindowSurfaces(this);
  await this.bridge.palette('shiftShift');
  await this.bridge.waitFor('the palette to open', () => this.bridge.state(), (s) => s.palette !== null && s.palette.focused);
});

Given('the user leaves the search field', async function (this: SapphireWorld) { await this.bridge.palette('blur'); });

When('the user opens the Command Palette via shift+shift', async function (this: SapphireWorld) {
  await this.bridge.palette('shiftShift');
});
When('the user presses Escape in the search field', async function (this: SapphireWorld) { await this.bridge.press('Escape'); });
When('the user filters commands by {string}', async function (this: SapphireWorld, text: string) { await this.bridge.palette('filter', text); });

Then('the Command Palette should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the palette to open', () => this.bridge.state(), (s) => s.palette !== null);
});
Then('the Command Palette should not be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the palette to close', () => this.bridge.state(), (s) => s.palette === null);
});
Then('the search field should be focused', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the palette search field to focus', () => this.bridge.state(), (s) => s.palette?.focused === true);
});
Then('the Command Palette should show {int} command(s)', async function (this: SapphireWorld, count: number) {
  await this.bridge.waitFor(`${count} commands`, () => this.bridge.state(), (s) => s.palette?.items.length === count);
});
Then('the Command Palette should show at least {int} command(s)', async function (this: SapphireWorld, count: number) {
  await this.bridge.waitFor(`at least ${count} commands`, () => this.bridge.state(), (s) => (s.palette?.items.length ?? 0) >= count);
});
Then('the selected command should be {string}', async function (this: SapphireWorld, title: string) {
  await this.bridge.waitFor(`the ${title} command to be selected`, () => this.bridge.state(), (s) => s.palette?.selected === title);
});
Then('the Workspace Hub overlay should not be present', async function (this: SapphireWorld) {
  assert.strictEqual((await this.bridge.state()).hub, null);
});
