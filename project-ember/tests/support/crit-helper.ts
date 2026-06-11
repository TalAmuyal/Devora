import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { DEFAULT_BUNDLE_DIR, findAppBundle } from '../../scripts/app-bundle';
import { GIT_TEST_IDENTITY } from './git-test-env';

/**
 * Create a git repo with a proper origin remote, suitable for
 * `create_workspace` (which runs `git fetch origin` and `git worktree add`).
 *
 * 1. Creates a bare repo in a temp dir (the "origin" remote)
 * 2. Creates the repo at `<profilePath>/repos/<repoName>/`
 * 3. Initialises with README.md and main.py, commits, and pushes to origin
 * 4. Sets refs/remotes/origin/HEAD so `git symbolic-ref` works
 */
export function createCritRepo(profilePath: string, repoName: string): { repoPath: string; bareRepoPath: string } {
  const gitEnv = { ...process.env, ...GIT_TEST_IDENTITY };

  // Bare repo as the "origin" remote
  const bareRepoPath = fs.mkdtempSync(path.join(os.tmpdir(), 'ember-crit-bare-'));
  execSync('git init --bare -b main', { cwd: bareRepoPath, stdio: 'ignore', env: gitEnv });

  // Repo under the profile's repos/ directory
  const repoPath = path.join(profilePath, 'repos', repoName);
  fs.mkdirSync(repoPath, { recursive: true });

  execSync('git init -b main', { cwd: repoPath, stdio: 'ignore', env: gitEnv });

  fs.writeFileSync(path.join(repoPath, 'README.md'), `# ${repoName}\n`);
  fs.writeFileSync(path.join(repoPath, 'main.py'), 'print("hello")\n');

  execSync('git add . && git commit -m "initial"', {
    cwd: repoPath,
    stdio: 'ignore',
    env: gitEnv,
  });

  execSync(`git remote add origin ${bareRepoPath}`, {
    cwd: repoPath,
    stdio: 'ignore',
    env: gitEnv,
  });

  execSync('git push -u origin main', {
    cwd: repoPath,
    stdio: 'ignore',
    env: gitEnv,
  });

  execSync('git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/main', {
    cwd: repoPath,
    stdio: 'ignore',
    env: gitEnv,
  });

  return { repoPath, bareRepoPath };
}

/**
 * Assert that the `original-crit` binary is available in the app bundle.
 */
export function assertOriginalCritAvailable(): void {
  const bundle = findAppBundle();
  if (bundle && fs.existsSync(path.join(bundle.resourcesDir, 'bundled-apps', 'original-crit'))) {
    return;
  }

  throw new Error(
    'original-crit not found in the app bundle\'s bundled-apps/ ' +
    `(searched ${DEFAULT_BUNDLE_DIR}). ` +
    'Run `mise test-e2e -- --force` (or `mise run build-ember-app` at the repo root) to rebuild the bundle.',
  );
}

