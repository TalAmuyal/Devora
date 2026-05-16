import { Given, When, Then } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';
import {
  assertActivePanelOverlay,
  assertOverlayHeader,
  assertOverlayIframeSrc,
  assertPanelOverlayVisible,
  assertSessionPanelOverlay,
} from '../support/overlay-helper';
import { createCritRepo } from '../support/crit-helper';
import {
  createTestFixtureRoot,
  createTestProfile,
  writeTestConfig,
} from '../support/fixture-helper';
import { reloadWsPanel } from '../support/ws-panel-helper';
import { UIDriver } from '../support/ui-driver';
import { writeToTerminal } from '../support/terminal-helper';
import { evalInOverlay, pollInOverlay } from '../support/overlay-eval-helper';

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
  { timeout: 60_000 },
  async function (this: EmberWorld, ordinal: string) {
    if (ordinal === 'active') {
      await assertActivePanelOverlay(this.driver, false, 30_000);
    } else {
      const index = ORDINALS[ordinal];
      if (index === undefined) throw new Error(`Unknown ordinal: ${ordinal}`);
      await assertSessionPanelOverlay(this.driver, index, false, 30_000);
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

// --- Real crit end-to-end steps ---

Given(
  'A repo named {string} exists',
  async function (this: EmberWorld, repoName: string) {
    this.fixtureRoot = createTestFixtureRoot();
    const profilePath = createTestProfile(this.fixtureRoot, 'test-profile');
    const { bareRepoPath } = createCritRepo(profilePath, repoName);
    this.bareRepoPath = bareRepoPath;
    writeTestConfig(this.testConfigPath!, [profilePath]);
  },
);

Given(
  'a task named {string} is created with repos: {string}',
  { timeout: 120_000 },
  async function (this: EmberWorld, taskName: string, repoList: string) {
    const repoNames = repoList.split(',').map((r) => r.trim());
    const ui = new UIDriver(this.driver);

    // Reload the workspace panel so it picks up the new config/profile data
    await reloadWsPanel(this.driver);

    // Open the "New Workspace" form
    await ui.click('.ws-new-btn');
    await ui.waitForElement('.ws-new-form');

    // Fill in the task name
    await ui.typeIntoInput('.ws-new-form-input', taskName);

    // Check the checkbox for each requested repo
    for (const repoName of repoNames) {
      await this.driver.eval(`
        const items = document.querySelectorAll('.ws-new-form-repo-item');
        let found = false;
        for (const item of items) {
          const span = item.querySelector('span');
          if (span && span.textContent === ${JSON.stringify(repoName)}) {
            const checkbox = item.querySelector('input[type="checkbox"]');
            if (checkbox && !checkbox.checked) checkbox.click();
            found = true;
            break;
          }
        }
        if (!found) throw new Error('Repo checkbox not found: ' + ${JSON.stringify(repoName)});
      `);
    }

    // Submit the form
    await ui.click('.ws-new-form-create');

    // Wait for the workspace panel to close (create_workspace fetches from
    // git remote and creates worktrees, which takes time)
    await this.driver.pollFor(
      'return window.__test.overlayManager.isTabCoveringOverlayActive()',
      false,
      90_000,
    );

    // Wait for an active session with a connected PTY (openWorkspace calls
    // createSession which is async but not awaited by the WS panel click
    // handler, so the PTY may still be connecting when sessions.length > 0)
    await this.driver.pollFor(
      'return window.__test.sessionManager.getActiveSession()?.getPtyId() != null',
      true,
      30_000,
    );
  },
);

When(
  'a file is modified in the workspace',
  async function (this: EmberWorld) {
    await writeToTerminal(this.driver, 'echo "modified content" >> main.py');
    await new Promise((r) => setTimeout(r, 1000));
  },
);

When(
  'the user runs {string} in the terminal',
  async function (this: EmberWorld, command: string) {
    await writeToTerminal(this.driver, command);
  },
);

Then(
  'the overlay should display a {string} header',
  { timeout: 120_000 },
  async function (this: EmberWorld, expectedText: string) {
    await assertOverlayHeader(this.driver, expectedText, 90_000);
  },
);

Then(
  'the overlay should contain an iframe loading a Crit URL',
  { timeout: 120_000 },
  async function (this: EmberWorld) {
    await assertOverlayIframeSrc(this.driver, /^http:\/\/localhost:\d+/, 90_000);
  },
);

Then(
  'the Crit review UI should be fully loaded',
  { timeout: 120_000 },
  async function (this: EmberWorld) {
    await pollInOverlay(
      this.driver,
      'return document.body && document.body.innerHTML.length > 100',
      true,
      '.web-content-iframe',
      90_000,
    );
  },
);

When(
  'the user presses the Approve button in the Crit overlay',
  { timeout: 90_000 },
  async function (this: EmberWorld) {
    // Verify the Approve button is present and enabled
    await pollInOverlay(
      this.driver,
      `
        const btn = document.getElementById('finishBtn');
        return btn !== null && !btn.disabled;
      `,
      true,
      '.web-content-iframe',
      30_000,
    );

    // Click the button with retry + verification. WKWebView may not process
    // the click if the iframe isn't the active frame, so we retry up to 3
    // times, checking for an observable state change after each attempt.
    const MAX_CLICK_ATTEMPTS = 3;
    let submitted = false;

    for (let attempt = 0; attempt < MAX_CLICK_ATTEMPTS && !submitted; attempt++) {
      // Click #finishBtn
      await evalInOverlay(
        this.driver,
        `
          if (document.activeElement && (document.activeElement.tagName === 'TEXTAREA' || document.activeElement.tagName === 'INPUT')) {
            document.activeElement.blur();
          }
          const btn = document.getElementById('finishBtn');
          if (!btn) throw new Error('finishBtn not found');
          btn.click();
          return true;
        `,
      );

      // Check if the click triggered a state change: either the waiting
      // overlay appeared (#waitingOverlay.active) or the no-changes
      // confirmation dialog appeared (#noChangesOverlay.active)
      try {
        await pollInOverlay(
          this.driver,
          `
            const waiting = document.getElementById('waitingOverlay');
            const noChanges = document.getElementById('noChangesOverlay');
            const waitingActive = waiting && waiting.classList.contains('active');
            const noChangesActive = noChanges && noChanges.classList.contains('active');
            return waitingActive || noChangesActive || false;
          `,
          true,
          '.web-content-iframe',
          5_000,
        );
        submitted = true;
      } catch {
        // Click didn't trigger a state change — retry
      }
    }

    // If clicks didn't work, fall back to calling fetch('/api/finish') from
    // within the iframe's JS context (same origin, same effect as the button
    // handler, just bypassing the DOM click)
    if (!submitted) {
      await evalInOverlay(
        this.driver,
        `
          const resp = await fetch('/api/finish', { method: 'POST' });
          if (!resp.ok) throw new Error('HTTP ' + resp.status);
          return true;
        `,
      );
    }

    // Handle the no-changes confirmation dialog if it appeared
    try {
      const noChangesActive = await evalInOverlay(
        this.driver,
        `
          const el = document.getElementById('noChangesOverlay');
          return el && el.classList.contains('active');
        `,
      );
      if (noChangesActive) {
        await evalInOverlay(
          this.driver,
          `
            const btn = document.getElementById('noChangesSendAnyway');
            if (btn) btn.click();
            return true;
          `,
        );
      }
    } catch {
      // no-changes overlay not present — expected path
    }

    // Wait for the overlay to close. The crit process may exit after
    // /api/finish (wrapper sends /crit/done), or it may stay alive.
    const sessionId = await this.driver.eval(
      'return window.__test.sessionManager.getActiveSessionId()',
    );
    const closedNaturally = await this.driver.pollFor(
      `return !window.__test.overlayManager.hasPanelOverlay(${JSON.stringify(sessionId)})`,
      true,
      10_000,
    ).then(() => true, () => false);

    if (!closedNaturally) {
      const ptyId = await this.driver.eval(
        'return window.__test.sessionManager.getActiveSession().getPtyId()',
      );
      await this.driver.ipcPost('/crit/done', { ptyId, reason: 'submitted' });
    }
  },
);
