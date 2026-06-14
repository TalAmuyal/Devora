import * as fs from 'node:fs';
import { Given } from '@cucumber/cucumber';
import { EmberWorld } from '../support/world';

// Writes a global config that turns the git-shortcut shims off.
// The Rust backend reads this file fresh on each create_pty, and the After hook resets it, so the override is scoped to the scenario.
Given('the global config disables git shortcuts', function (this: EmberWorld) {
  fs.writeFileSync(
    this.testConfigPath!,
    JSON.stringify({ profiles: [], terminal: { 'git-shortcuts': false } }),
  );
});
