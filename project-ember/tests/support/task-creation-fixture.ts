/**
 * The task-creation scenarios' fixtures and file-system checks, shared by Ember's and Sapphire's steps (`acceptance/features/task-creation.feature`).
 */

import assert from 'node:assert';
import { execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { GIT_TEST_IDENTITY } from './git-test-env';

export const PREPARE_MARKER = 'prepared.marker';

const GIT_ENV = { ...process.env, ...GIT_TEST_IDENTITY };

function git(args: string, cwd: string): string {
  return execSync(`git ${args}`, { cwd, env: GIT_ENV, stdio: ['ignore', 'pipe', 'ignore'] }).toString();
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

function workspaceDir(profilePath: string, wsId: string): string {
  return path.join(profilePath, 'workspaces', wsId);
}

function sourceRepo(profilePath: string, repoName: string): string {
  return path.join(profilePath, 'repos', repoName);
}

/**
 * A bare "origin" with one commit on main, cloned into <profile>/repos/<repo> so the source repo has the origin remote that fresh creation (fetch + worktree add origin/<default>) needs.
 * Repos under <profile>/repos/ are auto-discovered, so no config change is needed to register an extra one.
 */
export function createOriginBackedRepo(fixtureRoot: string, profilePath: string, repoName: string): void {
  const bare = path.join(fixtureRoot, `${repoName}.git`);
  fs.mkdirSync(bare, { recursive: true });
  git('init --bare -b main', bare);

  const source = sourceRepo(profilePath, repoName);
  git(`clone ${JSON.stringify(bare)} ${JSON.stringify(source)}`, fixtureRoot);
  fs.writeFileSync(path.join(source, 'file.txt'), 'v1\n');
  git('add .', source);
  git('commit -m c1', source);
  git('push origin main', source);
}

/** Sets the global config's `prepare-command`. */
export function setPrepareCommand(configPath: string, command: string): void {
  const config = JSON.parse(fs.readFileSync(configPath, 'utf-8'));
  config['prepare-command'] = command;
  fs.writeFileSync(configPath, JSON.stringify(config));
}

/** An initialized workspace with no task.json, so inactive, whose one worktree is a detached checkout of the repo's origin/main. */
export function createInactiveWorkspaceWithWorktree(profilePath: string, wsId: string, repoName: string): void {
  const ws = workspaceDir(profilePath, wsId);
  fs.mkdirSync(ws, { recursive: true });
  fs.writeFileSync(path.join(ws, 'initialized'), '');
  const worktree = path.join(ws, repoName);
  git(`worktree add --detach ${JSON.stringify(worktree)} origin/main`, sourceRepo(profilePath, repoName));
}

/** An untracked stand-in for a dependency cache (node_modules/.venv) that must survive a refresh. */
export function createCachedFile(profilePath: string, wsId: string, repoName: string, file: string): void {
  fs.writeFileSync(path.join(workspaceDir(profilePath, wsId), repoName, file), 'cache');
}

/** Pushes a new commit to the repo's origin; returns it. */
export function advanceOrigin(profilePath: string, repoName: string): string {
  const source = sourceRepo(profilePath, repoName);
  fs.writeFileSync(path.join(source, 'file.txt'), 'v2\n');
  git('add .', source);
  git('commit -m c2', source);
  git('push origin main', source);
  return git('rev-parse HEAD', source).trim();
}

export async function assertWorkspaceActiveWithTitle(profilePath: string, wsId: string, title: string): Promise<void> {
  const taskPath = path.join(workspaceDir(profilePath, wsId), 'task.json');
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    if (fs.existsSync(taskPath)) {
      const task = JSON.parse(fs.readFileSync(taskPath, 'utf-8'));
      if (task.title === title) return;
    }
    await sleep(100);
  }
  assert.fail(`workspace ${wsId} should be active with title "${title}"`);
}

export async function assertNoWorkspace(profilePath: string, wsId: string): Promise<void> {
  const ws = workspaceDir(profilePath, wsId);
  const deadline = Date.now() + 10_000;
  while (fs.existsSync(ws) && Date.now() < deadline) {
    await sleep(100);
  }
  assert.strictEqual(fs.existsSync(ws), false, `workspace ${wsId} should not exist`);
}

export async function assertWorkspaceInactive(profilePath: string, wsId: string): Promise<void> {
  const ws = workspaceDir(profilePath, wsId);
  const taskPath = path.join(ws, 'task.json');
  const deadline = Date.now() + 10_000;
  while (fs.existsSync(taskPath) && Date.now() < deadline) {
    await sleep(100);
  }
  assert.ok(fs.existsSync(ws), `workspace ${wsId} directory should still exist`);
  assert.strictEqual(fs.existsSync(taskPath), false, `workspace ${wsId} should be inactive (no task.json)`);
}

export function assertWorktreeAt(profilePath: string, wsId: string, repoName: string, expectedHead: string | undefined): void {
  const head = git('rev-parse HEAD', path.join(workspaceDir(profilePath, wsId), repoName)).trim();
  assert.strictEqual(head, expectedHead, 'worktree should be refreshed to the latest origin commit');
}

export function assertCachedFile(profilePath: string, wsId: string, repoName: string, file: string): void {
  const cachePath = path.join(workspaceDir(profilePath, wsId), repoName, file);
  assert.ok(fs.existsSync(cachePath), `cached file should survive: ${cachePath}`);
}

export async function assertPrepareMarker(profilePath: string, wsId: string, repoName: string): Promise<void> {
  const markerPath = path.join(workspaceDir(profilePath, wsId), repoName, PREPARE_MARKER);
  const deadline = Date.now() + 10_000;
  while (!fs.existsSync(markerPath) && Date.now() < deadline) {
    await sleep(100);
  }
  assert.ok(fs.existsSync(markerPath), `prepare marker should exist: ${markerPath}`);
}
