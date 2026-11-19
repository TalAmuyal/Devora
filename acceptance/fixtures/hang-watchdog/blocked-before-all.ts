import { BeforeAll } from '@cucumber/cucumber';
import { execFileSync } from 'node:child_process';

BeforeAll(function () {
  execFileSync('sleep', ['117']);
});
