import assert from 'node:assert';
import { When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { writeToTerminal, waitForTerminalContent } from '../support/terminal-helper';

Then(
  'there should be {int} session(s)',
  async function (this: EmberWorld, expected: number) {
    const count = await this.driver.eval(
      'return window.__test.sessionManager.getSessionCount()',
    );
    assert.strictEqual(count, expected);
  },
);

Then(
  'the active session title should be {string}',
  async function (this: EmberWorld, expected: string) {
    const title = await this.driver.eval(
      'return window.__test.sessionManager.getActiveSession()?.title',
    );
    assert.strictEqual(title, expected);
  },
);

When('the user switches to the previous session', async function (this: EmberWorld) {
  await this.driver.eval('window.__test.sessionManager.activatePrevious()');
});

When('the user switches to the first session', async function (this: EmberWorld) {
  await this.driver.eval(`
    const firstId = window.__test.sessionManager.getSessions()[0]?.id;
    window.__test.sessionManager.activateSession(firstId);
  `);
});

When('the user switches to the second session', async function (this: EmberWorld) {
  await this.driver.eval(`
    const secondId = window.__test.sessionManager.getSessions()[1]?.id;
    window.__test.sessionManager.activateSession(secondId);
  `);
});

When(
  'the user switches to the second session before the overlay opens',
  async function (this: EmberWorld) {
    await this.driver.eval(`
      const secondId = window.__test.sessionManager.getSessions()[1]?.id;
      window.__test.sessionManager.activateSession(secondId);
    `);
  },
);

Then('the first session should be active', async function (this: EmberWorld) {
  const activeId = await this.driver.eval(
    'return window.__test.sessionManager.getActiveSessionId()',
  );
  const firstId = await this.driver.eval(
    'return window.__test.sessionManager.getSessions()[0]?.id',
  );
  assert.strictEqual(activeId, firstId);
});

When(
  '{string} is typed in the terminal',
  async function (this: EmberWorld, text: string) {
    await writeToTerminal(this.driver, text);
  },
);

Then(
  'the terminal should contain {string}',
  async function (this: EmberWorld, expected: string) {
    await waitForTerminalContent(this.driver, expected, 10_000);
  },
);
