import fs from 'node:fs';
import { Given } from '@cucumber/cucumber';
import { SapphireWorld } from '../support/world';

// The real core rereads config when it builds each terminal's launch spec.
Given('the global config disables git shortcuts', function (this: SapphireWorld) {
  const config = JSON.parse(fs.readFileSync(this.testConfigPath, 'utf8'));
  config.terminal = { ...config.terminal, 'git-shortcuts': false };
  fs.writeFileSync(this.testConfigPath, JSON.stringify(config));
});
