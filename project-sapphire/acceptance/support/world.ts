import { World, setWorldConstructor, setDefaultTimeout } from '@cucumber/cucumber';
import type { ChildProcess } from 'node:child_process';
import { launch, type LaunchedApp } from './app';
import type { AppState, Bridge, TabState } from './bridge';

/** What a restart must keep: the tabs' ids, titles and kinds, and the active tab. */
export interface RestartedTabs {
  tabs: Pick<TabState, 'id' | 'title' | 'kind'>[];
  activeTab: AppState['activeTab'];
}

setDefaultTimeout(30_000);

export class SapphireWorld extends World {
  app!: ChildProcess;
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
  fixtureRoot?: string;
  /** The profile the latest fixture step created. */
  profilePath?: string;

  /** Launches the app on the scenario's config and state; it becomes the app the steps drive. */
  async launch(): Promise<void> {
    const { app, bridge } = await launch(this.testConfigPath, this.stateDir);
    this.app = app;
    this.bridge = bridge;
  }
}

setWorldConstructor(SapphireWorld);
