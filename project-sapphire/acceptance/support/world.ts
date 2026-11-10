import { World, setWorldConstructor, setDefaultTimeout } from '@cucumber/cucumber';
import { claudeEnvironment } from './claude';
import type { ChildProcess } from 'node:child_process';
import { launch, BUNDLED_APP_BINARY, type LaunchedApp } from './app';
import type { AppState, Bridge, TabState } from './bridge';
import type { FailureCapture } from './failure-capture';

/** What a restart must keep: the tabs' ids, titles and kinds, and the active tab. */
export interface RestartedTabs {
  tabs: Pick<TabState, 'id' | 'title' | 'kind'>[];
  activeTab: AppState['activeTab'];
}

setDefaultTimeout(30_000);

export class SapphireWorld extends World {
  app!: ChildProcess;
  /** Bundled-tools scenarios use the signed distribution and an isolated login HOME. */
  bundledTools = false;
  /** Whether the app shows the native agent transcript (ADR-014), for `@native-transcript` scenarios. */
  nativeTranscript = false;
  home?: string;
  bridge!: Bridge;
  /** The scenario's own directory, holding its config and its app state. */
  scenarioDir!: string;
  /** The scenario's global config (`DEVORA_CONFIG_PATH`). */
  testConfigPath!: string;
  /** Where the app keeps its state (`DEVORA_STATE_DIR`), across the scenario's relaunches. */
  stateDir!: string;
  /** Another instance of the app, on the same state. */
  second?: LaunchedApp;
  /** The tabs, as the persistence steps compare them, before the app was restarted. */
  tabsBeforeRestart?: RestartedTabs;
  previewFiles: string[] = [];
  /** Each tab's PTY size, in tab order, as `stty size` reported it. */
  recordedSizes?: { rows: number; cols: number }[];
  failureCapture?: FailureCapture;
  workspacePath?: string;
  hookLogPath?: string;
  hookRelease?: string;
  claudeTab?: number;
  stopAutoApprove?: () => void;
  fixtureRoot?: string;
  /** The profile the latest fixture step created. */
  profilePath?: string;
  /** The profiles the fixture steps registered in the config, in order. */
  profilePaths: string[] = [];
  /** How many tabs were open before a task's creation opened its own. */
  sessionsBeforeTask?: number;
  /** The commit a refreshed worktree should be at. */
  expectedHead?: string;
	originalTaskUid?: string;
  /** Whether the app's key router consumed the latest key typed into a field. */
  lastKeyConsumed?: boolean;

  /** Launches the app on the scenario's config and state; it becomes the app the steps drive. */
  async launch(): Promise<void> {
    const { app, bridge } = await launch(this.testConfigPath, this.stateDir, { binary: this.bundledTools ? BUNDLED_APP_BINARY : undefined, home: this.home, environment: claudeEnvironment(), arguments: this.nativeTranscript ? ['-DevoraNativeTranscript', 'YES'] : [] });
    this.app = app;
    this.bridge = bridge;
  }
}

setWorldConstructor(SapphireWorld);
