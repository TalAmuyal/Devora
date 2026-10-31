import { Given, When } from '@cucumber/cucumber';
import * as path from 'node:path';
import {
  createSingleActiveWorkspace,
  createTestFixtureRoot,
  createTestProfile,
  createTestRepo,
  createTestWorkspaces,
  setDirectoryPrintingDefaultApp,
  writeTestConfig,
} from '../../../project-ember/tests/support/fixture-helper';
import { openWorkspaceHub } from './workspace-hub.steps';
import { SapphireWorld } from '../support/world';

const TEST_REPO_NAME = 'test-repo';

/** A profile whose workspaces are real worktrees of one repo, the first `active` of them with a task; it joins the profiles already in the config. */
function createProfile(world: SapphireWorld, name: string, count: number, active: number): void {
  world.fixtureRoot ??= createTestFixtureRoot();
  const profilePath = createTestProfile(world.fixtureRoot, name);
  createTestRepo(profilePath, TEST_REPO_NAME);
  createTestWorkspaces(profilePath, count, { active }, TEST_REPO_NAME);
  world.profilePaths.push(profilePath);
  writeTestConfig(world.testConfigPath, world.profilePaths);
  world.profilePath = profilePath;
}

Given('a profile {string} with {int} active workspaces', function (this: SapphireWorld, name: string, count: number) {
  createProfile(this, name, count, count);
});

// Ember's fixtures are real worktrees either way.
Given(
  'a profile {string} with {int} active workspace(s) with worktrees',
  function (this: SapphireWorld, name: string, count: number) {
    createProfile(this, name, count, count);
  },
);

Given(
  'a profile {string} with {int} active and {int} inactive workspaces',
  function (this: SapphireWorld, name: string, active: number, inactive: number) {
    createProfile(this, name, active + inactive, active);
  },
);

When('a second active workspace is added to profile {string}', function (this: SapphireWorld, name: string) {
  createSingleActiveWorkspace(path.join(this.fixtureRoot!, name), 'ws-2', 'Task 2', TEST_REPO_NAME);
});

Given("the profile's default app prints the directory it starts in", function (this: SapphireWorld) {
  setDirectoryPrintingDefaultApp(this.profilePath!);
});

When('the user opens the workspace {string}', async function (this: SapphireWorld, id: string) {
  // A freshly opened hub has its first workspace focused.
  const hub = await openWorkspaceHub(this);
  const index = hub.items.findIndex((item) => item.id === id);
  for (let i = 0; i < index; i++) await this.bridge.press('j');
  await this.bridge.waitFor(`${id} to be focused`, () => this.bridge.state(), (s) => s.hub?.focused === id);
  await this.bridge.press('Return');
  await this.bridge.waitFor(
    `a tab on workspace ${id} to be active`,
    () => this.bridge.state(),
    (s) => s.tabs.some((tab) => tab.id === s.activeTab && tab.kind === 'workspace'),
  );
});
