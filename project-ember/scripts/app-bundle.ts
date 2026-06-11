/**
 * Locates the built Ember app artifact and checks it is safe to test against:
 * present, complete (bundled-apps/), and fresh (BUILD_FINGERPRINT matches the
 * working tree). Shared by the acceptance-test harness and the freshness CLI.
 *
 * The fingerprint itself is computed by the bundler's single implementation
 * (bundler/macos-ember/bundle-fingerprint.sh) -- this module only invokes it.
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

const PROJECT_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const REPO_ROOT = path.resolve(PROJECT_ROOT, '..');
const FINGERPRINT_SCRIPT = path.join(REPO_ROOT, 'bundler/macos-ember/bundle-fingerprint.sh');
export const DEFAULT_BUNDLE_DIR = path.join(PROJECT_ROOT, 'src-tauri/target/release/bundle/macos');

export interface AppBundle {
  appDir: string;
  binaryPath: string;
  resourcesDir: string;
}

/** The bundled-apps/ entries acceptance tests rely on (subset of what populate-app-resources.sh installs). */
export const REQUIRED_BUNDLED_APPS = ['ccc', 'crit', 'original-crit', 'debi'];

export function findAppBundle(bundleDir: string = DEFAULT_BUNDLE_DIR): AppBundle | null {
  if (!fs.existsSync(bundleDir)) return null;
  const appDirs = fs.readdirSync(bundleDir).filter((e) => e.endsWith('.app'));
  if (appDirs.length === 0) return null;
  if (appDirs.length > 1) {
    throw new Error(
      `Multiple .app bundles found in ${bundleDir}: ${appDirs.join(', ')}. ` +
        'Delete stale bundles and rebuild.',
    );
  }
  const appDir = path.join(bundleDir, appDirs[0]);
  const binaryPath = path.join(appDir, 'Contents', 'MacOS', 'devora-ember');
  if (!fs.existsSync(binaryPath)) return null;
  return {
    appDir,
    binaryPath,
    resourcesDir: path.join(appDir, 'Contents', 'Resources'),
  };
}

export function findRawBinary(projectRoot: string = PROJECT_ROOT): string | null {
  const releasePath = path.join(projectRoot, 'src-tauri/target/release/devora-ember');
  const debugPath = path.join(projectRoot, 'src-tauri/target/debug/devora-ember');
  if (fs.existsSync(releasePath)) return releasePath;
  if (fs.existsSync(debugPath)) return debugPath;
  return null;
}

export function readBundleFingerprint(resourcesDir: string): string | null {
  const fingerprintPath = path.join(resourcesDir, 'BUILD_FINGERPRINT');
  if (!fs.existsSync(fingerprintPath)) return null;
  return fs.readFileSync(fingerprintPath, 'utf8').trim();
}

export function computeCurrentFingerprint(): string {
  return execFileSync(FINGERPRINT_SCRIPT, { encoding: 'utf8' }).trim();
}

export function assertBundleComplete(resourcesDir: string): void {
  const bundledAppsDir = path.join(resourcesDir, 'bundled-apps');
  const missing = REQUIRED_BUNDLED_APPS.filter(
    (name) => !fs.existsSync(path.join(bundledAppsDir, name)),
  );
  if (missing.length > 0) {
    throw new Error(
      `App bundle is incomplete -- missing from ${bundledAppsDir}: ${missing.join(', ')}.\n` +
        'The bundle was likely built without populate-app-resources.sh. ' +
        'Run `mise test-e2e -- --force` (or `mise run build-ember-app` at the repo root) to rebuild it properly.',
    );
  }
}
