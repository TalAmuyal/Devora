import { describe, it, expect, afterEach } from 'vitest';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  findAppBundle,
  findRawBinary,
  readBundleFingerprint,
  computeCurrentFingerprint,
  assertBundleComplete,
  REQUIRED_BUNDLED_APPS,
} from '../app-bundle';

const PROJECT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const REPO_ROOT = path.resolve(PROJECT_ROOT, '..');
const FINGERPRINT_SCRIPT = path.join(REPO_ROOT, 'bundler/macos-ember/bundle-fingerprint.sh');

const tempDirs: string[] = [];

function makeTempDir(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'app-bundle-test-'));
  tempDirs.push(dir);
  return dir;
}

function makeApp(
  bundleDir: string,
  name: string,
  options: { binary?: boolean } = {},
): { appDir: string; resourcesDir: string } {
  const appDir = path.join(bundleDir, name);
  const resourcesDir = path.join(appDir, 'Contents', 'Resources');
  fs.mkdirSync(path.join(appDir, 'Contents', 'MacOS'), { recursive: true });
  fs.mkdirSync(resourcesDir, { recursive: true });
  if (options.binary !== false) {
    fs.writeFileSync(path.join(appDir, 'Contents', 'MacOS', 'devora-ember'), 'fake binary');
  }
  return { appDir, resourcesDir };
}

function makeCompleteBundledApps(resourcesDir: string): void {
  const bundledAppsDir = path.join(resourcesDir, 'bundled-apps');
  fs.mkdirSync(bundledAppsDir, { recursive: true });
  for (const name of REQUIRED_BUNDLED_APPS) {
    fs.writeFileSync(path.join(bundledAppsDir, name), 'fake tool');
  }
}

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe('findAppBundle', () => {
  it('returns null when the bundle directory does not exist', () => {
    expect(findAppBundle(path.join(makeTempDir(), 'nope'))).toBeNull();
  });

  it('returns null when the directory contains no .app', () => {
    expect(findAppBundle(makeTempDir())).toBeNull();
  });

  it('returns the bundle paths when a valid .app is present', () => {
    const bundleDir = makeTempDir();
    const { appDir, resourcesDir } = makeApp(bundleDir, 'Devora-Ember.app');

    const bundle = findAppBundle(bundleDir);
    expect(bundle).not.toBeNull();
    expect(bundle!.appDir).toBe(appDir);
    expect(bundle!.binaryPath).toBe(path.join(appDir, 'Contents', 'MacOS', 'devora-ember'));
    expect(bundle!.resourcesDir).toBe(resourcesDir);
  });

  it('returns null when the .app lacks the devora-ember binary', () => {
    const bundleDir = makeTempDir();
    makeApp(bundleDir, 'Devora-Ember.app', { binary: false });
    expect(findAppBundle(bundleDir)).toBeNull();
  });

  it('throws when multiple .app bundles are present', () => {
    const bundleDir = makeTempDir();
    makeApp(bundleDir, 'One.app');
    makeApp(bundleDir, 'Two.app');
    expect(() => findAppBundle(bundleDir)).toThrow(/Multiple \.app bundles/);
  });
});

describe('findRawBinary', () => {
  it('returns null when no raw binary exists', () => {
    expect(findRawBinary(makeTempDir())).toBeNull();
  });

  it('prefers the release binary over the debug one', () => {
    const projectRoot = makeTempDir();
    const release = path.join(projectRoot, 'src-tauri/target/release/devora-ember');
    const debug = path.join(projectRoot, 'src-tauri/target/debug/devora-ember');
    fs.mkdirSync(path.dirname(release), { recursive: true });
    fs.mkdirSync(path.dirname(debug), { recursive: true });
    fs.writeFileSync(release, 'release');
    fs.writeFileSync(debug, 'debug');

    expect(findRawBinary(projectRoot)).toBe(release);
  });

  it('falls back to the debug binary', () => {
    const projectRoot = makeTempDir();
    const debug = path.join(projectRoot, 'src-tauri/target/debug/devora-ember');
    fs.mkdirSync(path.dirname(debug), { recursive: true });
    fs.writeFileSync(debug, 'debug');

    expect(findRawBinary(projectRoot)).toBe(debug);
  });
});

describe('readBundleFingerprint', () => {
  it('returns the fingerprint when the resource exists', () => {
    const { resourcesDir } = makeApp(makeTempDir(), 'Devora-Ember.app');
    fs.writeFileSync(path.join(resourcesDir, 'BUILD_FINGERPRINT'), 'abc123');
    expect(readBundleFingerprint(resourcesDir)).toBe('abc123');
  });

  it('returns null when the resource is absent', () => {
    const { resourcesDir } = makeApp(makeTempDir(), 'Devora-Ember.app');
    expect(readBundleFingerprint(resourcesDir)).toBeNull();
  });
});

describe('computeCurrentFingerprint', () => {
  it('returns the same digest as invoking the bundler script directly', () => {
    const direct = execFileSync(FINGERPRINT_SCRIPT, { encoding: 'utf8' }).trim();
    expect(computeCurrentFingerprint()).toBe(direct);
  });
});

describe('assertBundleComplete', () => {
  it('passes when all required bundled apps are present', () => {
    const { resourcesDir } = makeApp(makeTempDir(), 'Devora-Ember.app');
    makeCompleteBundledApps(resourcesDir);
    expect(() => assertBundleComplete(resourcesDir)).not.toThrow();
  });

  it('throws naming every missing bundled app', () => {
    const { resourcesDir } = makeApp(makeTempDir(), 'Devora-Ember.app');
    makeCompleteBundledApps(resourcesDir);
    fs.rmSync(path.join(resourcesDir, 'bundled-apps', 'original-crit'));
    fs.rmSync(path.join(resourcesDir, 'bundled-apps', 'debi'));

    expect(() => assertBundleComplete(resourcesDir)).toThrow(/original-crit.*debi|debi.*original-crit/s);
    expect(() => assertBundleComplete(resourcesDir)).toThrow(/mise test-e2e -- --force/);
  });

  it('throws when bundled-apps/ is missing entirely', () => {
    const { resourcesDir } = makeApp(makeTempDir(), 'Devora-Ember.app');
    expect(() => assertBundleComplete(resourcesDir)).toThrow(/ccc/);
  });
});
