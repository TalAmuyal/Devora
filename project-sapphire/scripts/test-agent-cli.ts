import { spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, renameSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const sapphire = fileURLToPath(new URL('..', import.meta.url));
const workflow = readFileSync(path.join(sapphire, '../.github/workflows/ci.yml'), 'utf8');
const pin = workflow.match(/^\s*CLAUDE_CODE_VERSION:\s*"([\d.]+)"\s*$/m)?.[1];
if (!pin) throw new Error('CI must supply an exact CLAUDE_CODE_VERSION');
const cacheRoot = path.join(sapphire, 'build/agent-cli');
const cache = path.join(cacheRoot, pin);
const executable = (directory: string) => path.join(directory, 'node_modules/.bin/claude');

function matchesPin(directory: string): boolean {
  const result = spawnSync(executable(directory), ['--version'], { encoding: 'utf8' });
  return result.status === 0 && result.stdout.trim().split(/\s+/)[0] === pin;
}

if (!matchesPin(cache)) {
  mkdirSync(cacheRoot, { recursive: true });
  const installing = mkdtempSync(path.join(cacheRoot, `.${pin}-`));
  try {
    const result = spawnSync('npm', [
      'install', '--prefix', installing, '--no-save', '--no-package-lock',
      '--no-audit', '--no-fund', `@anthropic-ai/claude-code@${pin}`,
    ], { stdio: 'inherit' });
    if (result.status !== 0) throw new Error('Installing the pinned Claude CLI failed');
    if (!matchesPin(installing)) throw new Error('Installed Claude CLI does not match the CI pin');
    // A concurrent runner may have published the same immutable cache already.
    try {
      renameSync(installing, cache);
    } catch (error) {
      if (!matchesPin(cache)) throw error;
    }
  } finally {
    rmSync(installing, { recursive: true, force: true });
  }
}

const result = spawnSync('cargo', [
  'test', '--manifest-path', '../project-core/Cargo.toml', '-p', 'devora-agent',
  '--test', 'session_cli', '--', '--ignored', '--test-threads=1',
], {
  cwd: sapphire,
  env: { ...process.env, DEVORA_TEST_CLAUDE_PROGRAM: executable(cache) },
  stdio: 'inherit',
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
