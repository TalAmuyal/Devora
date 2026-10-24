import assert from 'node:assert';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { When, Then } from '@cucumber/cucumber';
import { captureFailure } from '../support/failure-capture';
import { EmberWorld } from '../support/world';

const PNG_SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

When('a failure is captured', async function (this: EmberWorld) {
  // A temp dir rather than the real capture dir, so CI artifacts only hold genuine failures.
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-failure-capture-'));
  this.failureCapture = await captureFailure(this.driver, 'failure-capture-scenario', dir);
});

Then('the capture should include a screenshot', function (this: EmberWorld) {
  const screenshot = this.failureCapture?.screenshot;
  assert.ok(screenshot, 'No screenshot was captured');
  const header = fs.readFileSync(screenshot).subarray(0, PNG_SIGNATURE.length);
  assert.ok(header.equals(PNG_SIGNATURE), `${screenshot} is not a PNG image`);
});

Then('the capture should include the terminal text {string}', function (this: EmberWorld, text: string) {
  const terminal = this.failureCapture?.terminal;
  assert.ok(terminal, 'No terminal text was captured');
  assert.ok(fs.readFileSync(terminal, 'utf-8').includes(text), `${terminal} does not contain "${text}"`);
});
