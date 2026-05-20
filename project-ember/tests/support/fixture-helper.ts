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

export function createTestWorkspaces(
  profilePath: string,
  count: number,
  options: { active?: number },
): WorkspaceFixture[] {
  const activeCount = options.active ?? count;
  const workspacesDir = path.join(profilePath, 'workspaces');
  fs.mkdirSync(workspacesDir, { recursive: true });

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

    // Dummy repo subdirectory so list_repo_subdirs finds something.
    // Needs a .git dir to look like a repo/worktree.
    const dummyRepoDir = path.join(wsPath, 'test-repo');
    fs.mkdirSync(path.join(dummyRepoDir, '.git'), { recursive: true });

    fixtures.push({
      id: wsId,
      path: wsPath,
      active: isActive,
      title: isActive ? title : '',
    });
  }

  return fixtures;
}

export function createTestWorkspacesWithRealRepos(
  profilePath: string,
  count: number,
  options: { active?: number },
): WorkspaceFixture[] {
  const activeCount = options.active ?? count;
  const workspacesDir = path.join(profilePath, 'workspaces');
  fs.mkdirSync(workspacesDir, { recursive: true });

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

    // Real git repo so git operations work during tests.
    const repoDir = path.join(wsPath, 'test-repo');
    fs.mkdirSync(repoDir, { recursive: true });
    execSync('git init', { cwd: repoDir, stdio: 'ignore' });
    fs.writeFileSync(path.join(repoDir, 'README.md'), `# test-repo\n`);
    execSync('git add . && git commit -m "init"', {
      cwd: repoDir,
      stdio: 'ignore',
      env: {
        ...process.env,
        GIT_AUTHOR_NAME: 'Test',
        GIT_AUTHOR_EMAIL: 'test@test.local',
        GIT_COMMITTER_NAME: 'Test',
        GIT_COMMITTER_EMAIL: 'test@test.local',
      },
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

export function writeTestConfig(configPath: string, profilePaths: string[]): void {
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  fs.writeFileSync(
    configPath,
    JSON.stringify({ profiles: profilePaths }),
  );
}

export function cleanupFixtures(fixtureRoot: string): void {
  fs.rmSync(fixtureRoot, { recursive: true, force: true });
}
