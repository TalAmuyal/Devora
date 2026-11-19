import { When } from '@cucumber/cucumber';
import { execFileSync } from 'node:child_process';
// Relative to where the test copies it: <project>/test-results/hang-watchdog-fixture/.
import { hangWatchdog } from '../../../acceptance/hang-watchdog';

// 117 s: long enough to outlast the test's 3 s limit, and a duration no other process is likely to sleep for, which the test looks for.
When('a step waits synchronously for a child process that doesn\'t end', function () {
  // As the harness reports the app's stderr, through its own import of the watchdog.
  hangWatchdog().output('The fixture\'s output', 'the last line it printed\n');
  execFileSync('sleep', ['117']);
});

When('a step leaves something for the process to wait for', function () {
  setInterval(() => {}, 1000);
});

When('a step passes', function () {});
