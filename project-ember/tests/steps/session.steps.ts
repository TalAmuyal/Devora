import assert from 'node:assert';
import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import { writeToTerminal, waitForTerminalContent } from '../support/terminal-helper';

// Both poll: session creation and retitling finish asynchronously after the steps that trigger them return (e.g. opening a workspace dismisses the Hub before its session is added).
Then(
  'there should be {int} session(s)',
  async function (this: EmberWorld, expected: number) {
    await this.driver.pollFor('return window.__test.sessionManager.getSessionCount()', expected, 5_000);
  },
);

Then(
  'the active session title should be {string}',
  async function (this: EmberWorld, expected: string) {
    await this.driver.pollFor('return window.__test.sessionManager.getActiveSession()?.title', expected, 5_000);
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

Then('the first session should be active', async function (this: EmberWorld) {
  const activeId = await this.driver.eval(
    'return window.__test.sessionManager.getActiveSessionId()',
  );
  const firstId = await this.driver.eval(
    'return window.__test.sessionManager.getSessions()[0]?.id',
  );
  assert.strictEqual(activeId, firstId);
});

Then('the third session should be active', async function (this: EmberWorld) {
  await this.driver.pollFor(
    `const m = window.__test.sessionManager; return m.getActiveSessionId() === m.getSessions()[2]?.id`,
    true,
    5_000,
  );
});

// Session ids grow with each new session, so a session's rank among them is its creation order, counted from 1.
Then(
  'the sessions should be in the order {int}, {int}, {int}',
  async function (this: EmberWorld, a: number, b: number, c: number) {
    const ids: number[] = await this.driver.eval(
      'return window.__test.sessionManager.getSessions().map((s) => s.id)',
    );
    const created = [...ids].sort((x, y) => x - y);
    assert.deepStrictEqual(ids.map((id) => created.indexOf(id) + 1), [a, b, c]);
  },
);

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

Given(
  "the active session's terminal width is recorded",
  async function (this: EmberWorld) {
    this.recordedTerminalCols = await this.driver.eval(`
      const session = window.__test.sessionManager.getActiveSession();
      return session.containerEl.__xtermTerminal.cols;
    `);
  },
);

When('the terminal font size is decreased', async function (this: EmberWorld) {
  // Dispatch the real Ctrl+Shift+Minus shortcut, which fits every session synchronously — including backgrounded ones — so the assertion needs no wait.
  await this.driver.eval(`
    window.dispatchEvent(new KeyboardEvent('keydown', {
      key: '-', code: 'Minus', ctrlKey: true, shiftKey: true,
      bubbles: true, cancelable: true,
    }));
  `);
});

Then(
  "the recorded session's terminal width should be unchanged",
  async function (this: EmberWorld) {
    const cols = await this.driver.eval(`
      const session = window.__test.sessionManager.getSessions()[0];
      return session.containerEl.__xtermTerminal.cols;
    `);
    assert.strictEqual(cols, this.recordedTerminalCols);
  },
);

const ORDINALS: Record<string, number> = { first: 0, second: 1, third: 2 };

Then(
  'the {word} session should have a panel overlay',
  async function (this: EmberWorld, ordinal: string) {
    const index = ORDINALS[ordinal];
    if (index === undefined) throw new Error(`Unknown ordinal: ${ordinal}`);
    await this.driver.pollFor(
      `return window.__test.sessionManager.getSessions()[${index}]?.hasSurface() ?? false`,
      true,
      10_000,
    );
  },
);

/**
 * Whether a session-scoped surface is actually on screen.
 * Asked of the DOM rather than of app state: a tab's surface is hidden by hiding the tab, and this is the assertion that would notice if that ever stopped being true.
 */
async function isPanelOverlayVisible(world: EmberWorld): Promise<boolean> {
  return world.driver.eval(
    `return Array.from(document.querySelectorAll('.session-content .layer-page'))
       .some((el) => el.getClientRects().length > 0)`,
  );
}

Then('the panel overlay should be visible', async function (this: EmberWorld) {
  assert.strictEqual(await isPanelOverlayVisible(this), true, 'Expected the panel overlay to be visible');
});

Then('the panel overlay should not be visible', async function (this: EmberWorld) {
  assert.strictEqual(await isPanelOverlayVisible(this), false, 'Expected the panel overlay to be hidden');
});

Given("the active session's terminal holds focus", async function (this: EmberWorld) {
  await this.driver.eval('window.__test.sessionManager.getActiveSession().terminalPane.focus()');
});
