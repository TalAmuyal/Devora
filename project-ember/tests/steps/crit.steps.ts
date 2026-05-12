import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import {
  assertActivePanelOverlay,
  assertPanelOverlayVisible,
  assertSessionPanelOverlay,
} from '../support/overlay-helper';

const ORDINALS: Record<string, number> = { first: 0, second: 1, third: 2 };

async function getActivePtyId(world: EmberWorld): Promise<number> {
  return await world.driver.eval(
    'return window.__test.sessionManager.getActiveSession().getPtyId()',
  );
}

async function getSessionPtyId(world: EmberWorld, index: number): Promise<number> {
  return await world.driver.eval(
    `return window.__test.sessionManager.getSessions()[${index}]?.getPtyId()`,
  );
}

async function sendCritOpen(world: EmberWorld, url: string, ptyId?: number): Promise<void> {
  const id = ptyId ?? (await getActivePtyId(world));
  world.driver.ipcPostFireAndForget('/crit/open', { ptyId: id, url });
  await new Promise((r) => setTimeout(r, 200));
}

When(
  'an external tool sends a crit-open request for the {word} session with URL {string}',
  async function (this: EmberWorld, target: string, url: string) {
    if (target === 'active') {
      await sendCritOpen(this, url);
    } else {
      const index = ORDINALS[target];
      if (index === undefined) throw new Error(`Unknown ordinal: ${target}`);
      const ptyId = await getSessionPtyId(this, index);
      await sendCritOpen(this, url, ptyId);
    }
  },
);

Then(
  'the {word} session should have a panel overlay',
  { timeout: 120_000 },
  async function (this: EmberWorld, ordinal: string) {
    if (ordinal === 'active') {
      await assertActivePanelOverlay(this.driver, true, 90_000);
    } else {
      const index = ORDINALS[ordinal];
      if (index === undefined) throw new Error(`Unknown ordinal: ${ordinal}`);
      await assertSessionPanelOverlay(this.driver, index, true, 90_000);
    }
  },
);

Then(
  'the {word} session should not have a panel overlay',
  async function (this: EmberWorld, ordinal: string) {
    if (ordinal === 'active') {
      await assertActivePanelOverlay(this.driver, false);
    } else {
      const index = ORDINALS[ordinal];
      if (index === undefined) throw new Error(`Unknown ordinal: ${ordinal}`);
      await assertSessionPanelOverlay(this.driver, index, false, 5_000);
    }
  },
);

Given(
  'the active session has a crit overlay',
  async function (this: EmberWorld) {
    await sendCritOpen(this, 'http://example.com/review');
    await assertActivePanelOverlay(this.driver, true);
  },
);

When(
  'an external tool sends a crit-done request for the active session',
  async function (this: EmberWorld) {
    const ptyId = await getActivePtyId(this);
    await this.driver.ipcPost('/crit/done', { ptyId, reason: 'completed' });
  },
);

Then(
  'the panel overlay should be visible',
  async function (this: EmberWorld) {
    await assertPanelOverlayVisible(this.driver, true);
  },
);

Then(
  'the panel overlay should not be visible',
  async function (this: EmberWorld) {
    await assertPanelOverlayVisible(this.driver, false);
  },
);
