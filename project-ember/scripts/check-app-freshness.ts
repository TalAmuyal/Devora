/**
 * CLI for the `test-e2e` mise task: exits 0 when a built app bundle exists and
 * its BUILD_FINGERPRINT matches the working tree, 1 (with the reason on
 * stderr) when the bundle is missing, unfingerprinted, or stale.
 */

import {
  DEFAULT_BUNDLE_DIR,
  computeCurrentFingerprint,
  findAppBundle,
  readBundleFingerprint,
} from './app-bundle';

const bundle = findAppBundle();
if (!bundle) {
  console.error(`No app bundle found in ${DEFAULT_BUNDLE_DIR}`);
  process.exit(1);
}

const builtFingerprint = readBundleFingerprint(bundle.resourcesDir);
if (!builtFingerprint) {
  console.error(
    `App bundle at ${bundle.appDir} has no BUILD_FINGERPRINT ` +
      '(built before fingerprinting existed, or without populate-app-resources.sh)',
  );
  process.exit(1);
}

const currentFingerprint = computeCurrentFingerprint();
if (builtFingerprint !== currentFingerprint) {
  console.error(
    `App bundle is stale: bundle fingerprint ${builtFingerprint.slice(0, 12)}… ` +
      `does not match working tree ${currentFingerprint.slice(0, 12)}…`,
  );
  process.exit(1);
}

console.log(`App bundle is fresh (fingerprint ${currentFingerprint.slice(0, 12)}…)`);
