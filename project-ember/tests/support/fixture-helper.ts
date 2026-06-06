import { execSync } from 'node:child_process';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { GIT_TEST_IDENTITY } from './git-test-env';

export interface WorkspaceFixture {
  id: string;
  path: string;
  active: boolean;
  title: string;
}

export function createTestFixtureRoot(): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), 'ember-fixture-'));
}

export function createTestProfile(fixtureRoot: string, name: string): string {
  const profilePath = path.join(fixtureRoot, name);
  fs.mkdirSync(profilePath, { recursive: true });

  fs.writeFileSync(
    path.join(profilePath, 'config.json'),
    JSON.stringify({ name }),
  );

  fs.mkdirSync(path.join(profilePath, 'workspaces'));
  fs.mkdirSync(path.join(profilePath, 'repos'));

  return profilePath;
}

export function createTestRepo(profilePath: string, repoName: string): string {
  const repoPath = path.join(profilePath, 'repos', repoName);
  fs.mkdirSync(repoPath, { recursive: true });

  execSync('git init', { cwd: repoPath, stdio: 'ignore' });

  fs.writeFileSync(
    path.join(repoPath, 'README.md'),
    `# ${repoName}\n`,
  );

  execSync('git add . && git commit -m "init"', {
    cwd: repoPath,
    stdio: 'ignore',
    env: { ...process.env, ...GIT_TEST_IDENTITY },
  });

  return repoPath;
}

/**
 * Scaffold the parts shared by every discoverable workspace at
 * `<profilePath>/workspaces/<id>`: the directory, the `initialized` marker that
 * makes `list_workspaces` pick it up, and (for active workspaces) a `task.json`.
 * The repo subdirectory is left to the caller, since that's the only part that
 * differs between a workspace with a worktree and an invalid one (no repos).
 */
function scaffoldWorkspace(
  profilePath: string,
  id: string,
  active: boolean,
  title: string,
): WorkspaceFixture {
  const wsPath = path.join(profilePath, 'workspaces', id);
  fs.mkdirSync(wsPath, { recursive: true });

  // Required marker for list_workspaces
  fs.writeFileSync(path.join(wsPath, 'initialized'), '');

  if (active) {
    const task = {
      uid: crypto.randomUUID(),
      title,
      started_at: '2026-01-01T00:00:00Z',
    };
    fs.writeFileSync(path.join(wsPath, 'task.json'), JSON.stringify(task));
  }

  return { id, path: wsPath, active, title: active ? title : '' };
}

/**
 * Write a single discoverable workspace at `<profilePath>/workspaces/<id>` with a git worktree checked out from `<profilePath>/repos/<repoName>`.
 * This mirrors production, where each workspace holds a worktree per registered repo.
 * Inactive workspaces omit `task.json` (matching production discovery behaviour).
 */
function writeRealWorkspace(
  profilePath: string,
  id: string,
  active: boolean,
  title: string,
  repoName: string,
): WorkspaceFixture {
  const fixture = scaffoldWorkspace(profilePath, id, active, title);

  const sourceRepo = path.join(profilePath, 'repos', repoName);
  const worktreePath = path.join(fixture.path, repoName);
  execSync(`git worktree add --detach ${JSON.stringify(worktreePath)}`, {
    cwd: sourceRepo,
    stdio: 'ignore',
  });

  return fixture;
}

/**
 * Create a single active, discoverable workspace with a git worktree.
 *
 * Takes an explicit workspace id, title, and repo name instead of restarting numbering at
 * ws-1, so it can add a workspace alongside existing ones without clobbering
 * them. Useful for tests that add a workspace out-of-band (e.g. to verify
 * refresh).
 */
export function createSingleActiveWorkspace(
  profilePath: string,
  id: string,
  title: string,
  repoName: string,
): WorkspaceFixture {
  return writeRealWorkspace(profilePath, id, true, title, repoName);
}

export function writeTestConfig(configPath: string, profilePaths: string[]): void {
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  fs.writeFileSync(
    configPath,
    JSON.stringify({ profiles: profilePaths }),
  );
}

/**
 * Create `count` discoverable workspaces (`ws-1`..`ws-N`) with real git worktrees.
 * Source repos live under `<profile>/repos/` and worktrees are checked out into
 * `<profile>/workspaces/ws-N/<repo-name>/`, matching production structure.
 */
export function createTestWorkspaces(
  profilePath: string,
  count: number,
  options: { active?: number },
  repoName: string,
): WorkspaceFixture[] {
  const activeCount = options.active ?? count;
  const fixtures: WorkspaceFixture[] = [];
  for (let i = 1; i <= count; i++) {
    fixtures.push(writeRealWorkspace(profilePath, `ws-${i}`, i <= activeCount, `Task ${i}`, repoName));
  }

  return fixtures;
}

/**
 * Create bare workspace directories with no repos (inactive, no task.json).
 *
 * These workspaces have the `initialized` marker so they appear in
 * `list_workspaces`, but no repo subdirectories. The hub shows them as
 * inactive. Since there are no worktrees, `delete_workspace` can remove
 * the directory directly.
 */
export function createInvalidWorkspaces(
  profilePath: string,
  count: number,
): WorkspaceFixture[] {
  const fixtures: WorkspaceFixture[] = [];

  for (let i = 1; i <= count; i++) {
    // Inactive (no task.json) and intentionally no repo subdirectory — see the
    // doc comment above for why these appear as invalid.
    fixtures.push(scaffoldWorkspace(profilePath, `ws-${i}`, false, ''));
  }

  return fixtures;
}

export function cleanupFixtures(fixtureRoot: string): void {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}
