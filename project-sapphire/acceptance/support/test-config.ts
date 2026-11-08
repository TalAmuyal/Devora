import * as fs from 'node:fs';
import { writeTestConfig as writeSharedConfig } from '../../../project-ember/tests/support/fixture-helper';

/** Background repo mutation is opt-in for Sapphire test fixtures. */
export function writeTestConfig(configPath: string, profilePaths: string[]): void {
  writeSharedConfig(configPath, profilePaths);
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  config['repo-update'] = { enabled: false };
  fs.writeFileSync(configPath, JSON.stringify(config));
}
