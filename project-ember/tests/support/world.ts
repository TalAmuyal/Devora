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
  lastKeyDefaultPrevented?: boolean;
  originalTaskUid?: string;
  // Latest origin commit a reused worktree is expected to refresh to (set by the origin-advanced step).
  expectedHead?: string;
  // Terminal column count of a session captured while active, to assert it survives backgrounding.
  recordedTerminalCols?: number;
}

setWorldConstructor(EmberWorld);
