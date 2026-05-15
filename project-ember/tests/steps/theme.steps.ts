import assert from 'node:assert';
import { Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';

Then(
  'the document root should have CSS property {string}',
  async function (this: EmberWorld, property: string) {
    const value = await this.driver.eval(
      `return getComputedStyle(document.documentElement).getPropertyValue(${JSON.stringify(property)})`,
    );
    assert.ok(
      value && value.trim() !== '',
      `CSS property ${property} is not set on the document root`,
    );
  },
);
