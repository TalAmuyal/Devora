import { describe, it, expect, afterEach } from 'vitest';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const PROJECT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const REPO_ROOT = path.resolve(PROJECT_ROOT, '..');
const FINGERPRINT_SCRIPT = path.join(REPO_ROOT, 'bundler/macos-ember/bundle-fingerprint.sh');
const POPULATE_SCRIPT = path.join(REPO_ROOT, 'bundler/macos-ember/populate-app-resources.sh');

const tempDirs: string[] = [];

function makeGitRepo(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'fingerprint-test-'));
  tempDirs.push(dir);
  execFileSync('git', ['init', '-q'], { cwd: dir });
  return dir;
}

function write(repo: string, relPath: string, content: string): void {
  const abs = path.join(repo, relPath);
  fs.mkdirSync(path.dirname(abs), { recursive: true });
  fs.writeFileSync(abs, content);
}

function track(repo: string, ...relPaths: string[]): void {
  execFileSync('git', ['add', '--', ...relPaths], { cwd: repo });
}

function fingerprint(repoRoot: string, inputs: string[]): string {
  return execFileSync(FINGERPRINT_SCRIPT, ['--repo-root', repoRoot, '--inputs', ...inputs], {
    encoding: 'utf8',
  }).trim();
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe('bundle-fingerprint.sh with explicit inputs', () => {
  it('prints a sha256 hex digest and is deterministic across calls', () => {
    const repo = makeGitRepo();
    write(repo, 'src/a.txt', 'alpha');
    track(repo, 'src/a.txt');

    const first = fingerprint(repo, ['src']);
    const second = fingerprint(repo, ['src']);
    expect(first).toMatch(/^[0-9a-f]{64}$/);
    expect(second).toBe(first);
  });

  it('is independent of file creation order', () => {
    const repoA = makeGitRepo();
    write(repoA, 'src/a.txt', 'alpha');
    write(repoA, 'src/b.txt', 'beta');
    track(repoA, 'src/a.txt', 'src/b.txt');

    const repoB = makeGitRepo();
    write(repoB, 'src/b.txt', 'beta');
    write(repoB, 'src/a.txt', 'alpha');
    track(repoB, 'src/b.txt', 'src/a.txt');

    expect(fingerprint(repoA, ['src'])).toBe(fingerprint(repoB, ['src']));
  });

  it('changes when file content changes', () => {
    const repo = makeGitRepo();
    write(repo, 'src/a.txt', 'alpha');
    track(repo, 'src/a.txt');
    const before = fingerprint(repo, ['src']);

    write(repo, 'src/a.txt', 'ALPHA');
    expect(fingerprint(repo, ['src'])).not.toBe(before);
  });

  it('changes when a file is renamed', () => {
    const repoA = makeGitRepo();
    write(repoA, 'src/a.txt', 'same content');
    track(repoA, 'src/a.txt');

    const repoB = makeGitRepo();
    write(repoB, 'src/b.txt', 'same content');
    track(repoB, 'src/b.txt');

    expect(fingerprint(repoA, ['src'])).not.toBe(fingerprint(repoB, ['src']));
  });

  it('excludes gitignored files', () => {
    const repoA = makeGitRepo();
    write(repoA, '.gitignore', 'src/ignored.txt\n');
    write(repoA, 'src/a.txt', 'alpha');
    write(repoA, 'src/ignored.txt', 'build output');
    track(repoA, '.gitignore', 'src/a.txt');

    const repoB = makeGitRepo();
    write(repoB, '.gitignore', 'src/ignored.txt\n');
    write(repoB, 'src/a.txt', 'alpha');
    track(repoB, '.gitignore', 'src/a.txt');

    expect(fingerprint(repoA, ['src'])).toBe(fingerprint(repoB, ['src']));
  });

  it('includes untracked, non-ignored files', () => {
    const repo = makeGitRepo();
    write(repo, 'src/a.txt', 'alpha');
    track(repo, 'src/a.txt');
    const before = fingerprint(repo, ['src']);

    write(repo, 'src/new-untracked.txt', 'fresh edit');
    expect(fingerprint(repo, ['src'])).not.toBe(before);
  });

  it('skips tracked files that were deleted from disk', () => {
    const repoA = makeGitRepo();
    write(repoA, 'src/a.txt', 'alpha');
    write(repoA, 'src/gone.txt', 'soon deleted');
    track(repoA, 'src/a.txt', 'src/gone.txt');
    fs.rmSync(path.join(repoA, 'src/gone.txt'));

    const repoB = makeGitRepo();
    write(repoB, 'src/a.txt', 'alpha');
    track(repoB, 'src/a.txt');

    expect(fingerprint(repoA, ['src'])).toBe(fingerprint(repoB, ['src']));
  });

  it('handles filenames containing spaces', () => {
    const repo = makeGitRepo();
    write(repo, 'src/has space.txt', 'alpha');
    track(repo, 'src/has space.txt');
    const before = fingerprint(repo, ['src']);
    expect(before).toMatch(/^[0-9a-f]{64}$/);

    write(repo, 'src/has space.txt', 'beta');
    expect(fingerprint(repo, ['src'])).not.toBe(before);
  });

  it('only hashes files under the given inputs', () => {
    const repo = makeGitRepo();
    write(repo, 'src/a.txt', 'alpha');
    write(repo, 'unrelated/b.txt', 'beta');
    track(repo, 'src/a.txt', 'unrelated/b.txt');
    const before = fingerprint(repo, ['src']);

    write(repo, 'unrelated/b.txt', 'BETA');
    expect(fingerprint(repo, ['src'])).toBe(before);
  });
});

describe('bundle-fingerprint.sh default mode (real repo)', () => {
  it('prints a deterministic sha256 digest for the working tree', () => {
    const first = execFileSync(FINGERPRINT_SCRIPT, { encoding: 'utf8' }).trim();
    const second = execFileSync(FINGERPRINT_SCRIPT, { encoding: 'utf8' }).trim();
    expect(first).toMatch(/^[0-9a-f]{64}$/);
    expect(second).toBe(first);
  });
});

describe('populate-app-resources.sh --list-sources', () => {
  function listSources(): string[] {
    return execFileSync(POPULATE_SCRIPT, ['--list-sources'], { encoding: 'utf8' })
      .split('\n')
      .filter((line) => line !== '');
  }

  it('lists the repo paths that determine bundle content', () => {
    const sources = listSources();
    expect(sources.length).toBeGreaterThan(0);
    expect(sources).toContain('ccc.sh');
    expect(sources).toContain('project-debi');
    expect(sources).toContain('bundler/3rd-party-deps.json');
  });

  it('lists only paths that exist in the repo', () => {
    for (const source of listSources()) {
      expect(fs.existsSync(path.join(REPO_ROOT, source)), `missing: ${source}`).toBe(true);
    }
  });

  it('is stable across invocations', () => {
    expect(listSources()).toEqual(listSources());
  });
});
