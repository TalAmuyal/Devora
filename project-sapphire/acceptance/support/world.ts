import { World, setWorldConstructor, setDefaultTimeout } from '@cucumber/cucumber';
import type { ChildProcess } from 'node:child_process';
import type { Bridge } from './bridge';

setDefaultTimeout(30_000);

export class SapphireWorld extends World {
  app!: ChildProcess;
  bridge!: Bridge;
  /** The scenario's global config (`DEVORA_CONFIG_PATH`), in a directory of its own. */
  testConfigPath!: string;
  fixtureRoot?: string;
  /** The profile the latest fixture step created. */
  profilePath?: string;
}

setWorldConstructor(SapphireWorld);
