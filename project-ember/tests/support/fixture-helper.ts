import { execSync } from 'node:child_process';
import * as crypto from 'node:crypto';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

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
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: 'Test',
      GIT_AUTHOR_EMAIL: 'test@test.local',
      GIT_COMMITTER_NAME: 'Test',
      GIT_COMMITTER_EMAIL: 'test@test.local',
    },
  });

  return repoPath;
}

/**
 * Write a single workspace with a fake `.git` directory at `<profilePath>/workspaces/<id>`.
 * Inactive workspaces omit `task.json` (matching production discovery behaviour).
 */
function writeFakeWorkspace(
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

  // Dummy repo subdirectory so list_repo_subdirs finds something.
  // Needs a .git dir to look like a repo/worktree.
  const dummyRepoDir = path.join(wsPath, 'test-repo');
  fs.mkdirSync(path.join(dummyRepoDir, '.git'), { recursive: true });

  return { id, path: wsPath, active, title: active ? title : '' };
}

/**
 * Create workspaces with fake `.git` directories (no real git repos).
 * These are suitable for UI-only tests that need workspace entries in the hub
 * but don't perform any real git operations.
 */
export function createFakeTestWorkspaces(
  profilePath: string,
  count: number,
  options: { active?: number },
): WorkspaceFixture[] {
  const activeCount = options.active ?? count;
  fs.mkdirSync(path.join(profilePath, 'workspaces'), { recursive: true });

  const fixtures: WorkspaceFixture[] = [];
  for (let i = 1; i <= count; i++) {
    fixtures.push(writeFakeWorkspace(profilePath, `ws-${i}`, i <= activeCount, `Task ${i}`));
  }

  return fixtures;
}

/**
 * Create a single active, discoverable workspace with a fake `.git` directory.
 *
 * Takes an explicit workspace id and title instead of restarting numbering at
 * ws-1, so it can add a workspace alongside existing ones without clobbering
 * them. Useful for tests that add a workspace out-of-band (e.g. to verify
 * refresh).
 */
export function createSingleActiveWorkspace(
  profilePath: string,
  id: string,
  title: string,
): WorkspaceFixture {
  return writeFakeWorkspace(profilePath, id, true, title);
}

export function writeTestConfig(configPath: string, profilePaths: string[]): void {
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  fs.writeFileSync(
    configPath,
    JSON.stringify({ profiles: profilePaths }),
  );
}

/**
 * Create workspaces with real git worktrees from a source repo.
 * A real workspace always has worktrees — this mirrors production structure
 * where source repos live under `<profile>/repos/` and worktrees are checked
 * out into `<profile>/workspaces/ws-N/<repo-name>/`.
 * Use this for any test that involves git operations on workspaces.
 */
export function createRealTestWorkspaces(
  profilePath: string,
  count: number,
  options: { active?: number },
): WorkspaceFixture[] {
  const activeCount = options.active ?? count;
  const workspacesDir = path.join(profilePath, 'workspaces');
  fs.mkdirSync(workspacesDir, { recursive: true });

  const sourceRepo = path.join(profilePath, 'repos', 'test-repo');
  const fixtures: WorkspaceFixture[] = [];

  for (let i = 1; i <= count; i++) {
    const wsId = `ws-${i}`;
    const wsPath = path.join(workspacesDir, wsId);
    fs.mkdirSync(wsPath, { recursive: true });

    // Required marker for list_workspaces
    fs.writeFileSync(path.join(wsPath, 'initialized'), '');

    const isActive = i <= activeCount;
    const title = `Task ${i}`;

    if (isActive) {
      const task = {
        uid: crypto.randomUUID(),
        title,
        started_at: '2026-01-01T00:00:00Z',
      };
      fs.writeFileSync(
        path.join(wsPath, 'task.json'),
        JSON.stringify(task),
      );
    }

    // Create a real git worktree from the profile's source repo.
    const worktreePath = path.join(wsPath, 'test-repo');
    execSync(`git worktree add --detach ${JSON.stringify(worktreePath)}`, {
      cwd: sourceRepo,
      stdio: 'ignore',
    });

    fixtures.push({
      id: wsId,
      path: wsPath,
      active: isActive,
      title: isActive ? title : '',
    });
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
  const workspacesDir = path.join(profilePath, 'workspaces');
  fs.mkdirSync(workspacesDir, { recursive: true });

  const fixtures: WorkspaceFixture[] = [];

  for (let i = 1; i <= count; i++) {
    const wsId = `ws-${i}`;
    const wsPath = path.join(workspacesDir, wsId);
    fs.mkdirSync(wsPath, { recursive: true });

    // Marker required for list_workspaces to include this workspace
    fs.writeFileSync(path.join(wsPath, 'initialized'), '');

    // No task.json (inactive) and no repo subdirectories.

    fixtures.push({ id: wsId, path: wsPath, active: false, title: '' });
  }

  return fixtures;
}

export function cleanupFixtures(fixtureRoot: string): void {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}
