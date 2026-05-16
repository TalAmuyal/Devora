import { World, setWorldConstructor, setDefaultTimeout } from '@cucumber/cucumber';
import { AppDriver } from './app-driver';

setDefaultTimeout(30_000);

export class EmberWorld extends World {
  driver!: AppDriver;
  workspacePath?: string;
  hookLogPath?: string;
  fixtureRoot?: string;
  bareRepoPath?: string;
  testConfigPath?: string;
  stopAutoApprove?: () => void;
}

setWorldConstructor(EmberWorld);
