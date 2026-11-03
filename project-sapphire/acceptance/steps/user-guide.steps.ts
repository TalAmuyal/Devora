import assert from 'node:assert/strict';
import { When, Then } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';

When('the user presses F1', async function (this: SapphireWorld) {
  await this.bridge.press('F1');
});
Then('the User Guide should be visible', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the User Guide', () => this.bridge.state(), (s) => s.windowLayers.includes('user-guide'));
});
Then('the User Guide should be stacked above the dialog', async function (this: SapphireWorld) {
  const layers = (await this.bridge.state()).windowLayers;
  assert.ok(layers.indexOf('new-task-dialog') >= 0 && layers.indexOf('user-guide') > layers.indexOf('new-task-dialog'));
});
Then('the User Guide should be open only once', async function (this: SapphireWorld) {
  assert.equal((await this.bridge.state()).windowLayers.filter((layer) => layer === 'user-guide').length, 1);
});
Then('the User Guide should be closed and the dialog restored', async function (this: SapphireWorld) {
  await this.bridge.waitFor('the dialog to be restored', () => this.bridge.state(), (s) => !s.windowLayers.includes('user-guide') && s.windowLayers.at(-1) === 'new-task-dialog');
});
