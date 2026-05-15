import { Given, When } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';

Given('Ember is running', async function (this: EmberWorld) {
  await this.driver.pollFor(
    'return typeof window.__test?.sessionManager !== "undefined"',
    true,
    15_000,
  );
});

Given('a session exists', async function (this: EmberWorld) {
  await this.driver.eval(`await window.__test.sessionManager.createSession('session')`);
});

Given('{int} sessions exist', async function (this: EmberWorld, count: number) {
  for (let i = 0; i < count; i++) {
    await this.driver.eval(
      `await window.__test.sessionManager.createSession('session-${i + 1}')`,
    );
  }
});

When('a new session is created', async function (this: EmberWorld) {
  await this.driver.eval(`await window.__test.sessionManager.createSession('Shell')`);
});

When(
  'a new session is created with title {string}',
  async function (this: EmberWorld, title: string) {
    await this.driver.eval(
      `await window.__test.sessionManager.createSession(${JSON.stringify(title)})`,
    );
  },
);

When('the active session is closed', async function (this: EmberWorld) {
  await this.driver.eval(`
    const id = window.__test.sessionManager.getActiveSessionId();
    window.__test.sessionManager.closeSession(id);
  `);
});
