import { Given, When } from '@cucumber/cucumber';
import {
  createTestFixtureRoot,
  createTestProfile,
  createTestRepo,
  createTestWorkspaces,
  setDirectoryPrintingDefaultApp,
  writeTestConfig,
} from '../../../project-ember/tests/support/fixture-helper';
import { SapphireWorld } from '../support/world';

const TEST_REPO_NAME = 'test-repo';

Given('a profile {string} with {int} active workspaces', function (this: SapphireWorld, name: string, count: number) {
  this.fixtureRoot = createTestFixtureRoot();
  const profilePath = createTestProfile(this.fixtureRoot, name);
  createTestRepo(profilePath, TEST_REPO_NAME);
  createTestWorkspaces(profilePath, count, { active: count }, TEST_REPO_NAME);
  writeTestConfig(this.testConfigPath, [profilePath]);
  this.profilePath = profilePath;
});

Given("the profile's default app prints the directory it starts in", function (this: SapphireWorld) {
  setDirectoryPrintingDefaultApp(this.profilePath!);
});

When('the user opens the workspace {string}', async function (this: SapphireWorld, id: string) {
  // The sidebar lists what the app loaded at launch, before the scenario's fixtures existed; reload it, as Ember's steps reopen the Workspace Hub.
  await this.bridge.dispatch({ command: 'load' });
  await this.bridge.dispatch({ command: 'openWorkspace', id });
  await this.bridge.waitFor(
    `a tab on workspace ${id} to be active`,
    () => this.bridge.state(),
    (s) => s.tabs.some((tab) => tab.id === s.activeTab && tab.kind === 'workspace'),
  );
});
