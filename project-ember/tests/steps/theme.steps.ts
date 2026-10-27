import assert from 'node:assert';
import { Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';

Then(
  'the document root should have the theme CSS property {string}',
  async function (this: EmberWorld, property: string) {
    // Inline style, not computed: the stylesheet's defaults would satisfy a computed check even when the theme failed to load.
    const value = await this.driver.eval(
      `return document.documentElement.style.getPropertyValue(${JSON.stringify(property)})`,
    );
    assert.ok(
      value && value.trim() !== '',
      `theme CSS property ${property} is not set on the document root`,
    );
  },
);
