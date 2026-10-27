import { World, setWorldConstructor, setDefaultTimeout } from '@cucumber/cucumber';
import { AppDriver } from './app-driver';
import type { FailureCapture } from './failure-capture';

setDefaultTimeout(30_000);

export class EmberWorld extends World {
  driver!: AppDriver;
  workspacePath?: string;
  hookLogPath?: string;
  fixtureRoot?: string;
  testConfigPath?: string;
  stopAutoApprove?: () => void;
  lastKeyDefaultPrevented?: boolean;
  originalTaskUid?: string;
  // Latest origin commit a reused worktree is expected to refresh to (set by the origin-advanced step).
  expectedHead?: string;
  // Terminal column count of a session captured while active, to assert it survives backgrounding.
  recordedTerminalCols?: number;
  // Temp files created for preview scenarios; the last one is reused by "same file" steps.
  previewFiles?: string[];
  // Session count before a task was created, i.e. what remains once its creation tab closes.
  sessionsBeforeTask?: number;
  failureCapture?: FailureCapture;
}

setWorldConstructor(EmberWorld);
