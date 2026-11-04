# CD Pipeline

## Overview

The CD pipeline (`.github/workflows/cd.yml`) automates building and publishing Devora releases.
It publishes `Devora-Ember.dmg` containing `Devora Ember.app`; nightly releases also attach `Devora-Sapphire.dmg` containing `Devora Sapphire.app` under the same tag.

There are two release types: **nightly** builds that run daily, and **stable** releases triggered by version tags.

## Triggers

| Trigger | Schedule / Pattern | Release type |
|---|---|---|
| Cron | Daily at 03:00 UTC (midnight UTC-3) | Nightly |
| Tag push | `v*` (e.g. `v2026-03-28.0`) | Stable |
| PR merge | Release PR merged to `master` (auto-tag workflow) | Creates `v*` tag |
| Manual dispatch | `workflow_dispatch` via GitHub Actions UI or CLI | Depends on ref (tag → stable, branch → nightly) |

## Build process

`check-changes` gates unchanged scheduled nightlies and decides whether Sapphire is enabled for this channel.
`test` runs the shared suites on Linux; `test-ember` checks Ember on macOS 15.
`build-and-release` builds Ember on macOS 15 after both test jobs, then uploads its DMG artifact.
`build-sapphire` runs independently on `xcode-27`, with the same pinned Xcode 27.1 and GhosttyKit/Rust caches as CI.
It bundles through the shared resource script, smoke-tests the signed bundle via the acceptance bridge, creates a DMG with `hdiutil-create-retry.sh`, then uploads it.
`publish` downloads the enabled artifacts and attaches them to the same release.
With Sapphire disabled for stable, publishing Ember works even when the Sapphire job is skipped or unsuccessful.
Ember publishes on both channels even if Sapphire fails; the failed job stays visible, a warning is recorded in the run summary/release notes, and Sapphire is attached only on success.
After an incomplete Sapphire nightly, the marker stays unchanged so the next scheduled run retries that SHA.

**Stable Sapphire gate:** `SAPPHIRE_STABLE_ENABLED` at the top of the workflow is the single switch, currently `false` until Tal approves after the week-long daily-use gate.
Enabling that switch later also requires lifting `install.sh`'s explicit nightly-only Sapphire validation.
Do not dispatch CD to test changes without coordinating with Bravo; it publishes real releases.

### Build steps (build-and-release)

1. **Checkout** with `fetch-depth: 0` (full history, needed for version computation).
2. **Set up mise** for Go, Python, and other tool versions.
3. **Restore build caches** for the Ember Rust build artifacts (keyed on `Cargo.lock`) and npm modules (keyed on `package-lock.json`).
4. **Build the app bundle** via `mise run ember-bundle`, then **create the DMG** via `mise run ember-create-dmg`. This compiles Debi and Status-line, builds the Tauri (Ember) app into `Devora Ember.app`, bundles all resources, and produces a compressed `Devora-Ember.dmg`.
5. **Smoke test** -- verifies that:
   - The built `.app` exists and contains the `devora-ember` binary
   - Bundled resources (`debi`) were copied in
   - The version in `Info.plist` was patched (not the placeholder `0.1.0`)
   - The app icon (`app.icns`) was generated
   - `Devora-Ember.dmg` was created
6. **Upload the artifact for the publish job** (see [Release types](#release-types)).

## Release types

### Nightly

Nightly builds run on the daily cron schedule.
They produce two GitHub Releases:

- **Dated release** (`nightly-YYYY-MM-DD`) -- a snapshot for that specific day. If re-run on the same day, the existing release is deleted and recreated.
- **Rolling alias** (`nightly-latest`) -- always points to the most recent nightly. Deleted and recreated on each run.

Both are marked as **pre-release** and have `latest=false` so they never appear as the "Latest" release on GitHub.

Release notes include the short commit SHA and date, with a note that the build may be unstable.

#### Skip logic

Scheduled nightly runs are skipped when `master` has not advanced since the previous successful nightly.
A dedicated `check-changes` job compares `${{ github.sha }}` against the `nightly-last-built` git tag (force-pushed at the end of every successful nightly publish) using `git ls-remote`, and gates the `test` and `build-and-release` jobs on the result.
`workflow_dispatch` runs, tag pushes, and re-runs of scheduled runs always build.
If the marker tag is absent (cold start) or the lookup fails, the pipeline fails open and builds.

### Stable

Stable releases are triggered by pushing a tag matching `v*` (e.g. `v2026-03-28.0`).

The publish step extracts release notes from `CHANGELOG.md` by finding the section header matching the version (without the `v` prefix) and collecting content up to the next `##` header.
If no matching section is found, it falls back to a generic "Release VERSION" message.

Stable releases are **not** marked as pre-release and use the default `latest` behavior (the most recent stable release is shown as "Latest" on GitHub).

## How to cut a stable release

Run the release script via mise (or directly):

```bash
mise release                        # auto-generates a date-based version (YYYY-MM-DD.N)
mise release -- <version>           # provide an explicit version
mise release -- --no-ai             # skip the Claude Code editorial cleanup step
mise release -- <version> --no-ai   # both
```

Or equivalently:

```bash
./cut-release.sh
./cut-release.sh <version>
./cut-release.sh --no-ai
./cut-release.sh <version> --no-ai
```

The script:
1. Validates the working tree (clean, on `master`, up to date with remote)
2. Computes the new version (date-based by default, with an incrementing patch number for same-day releases)
3. Validates that the `## Unreleased` section in `CHANGELOG.md` has content
4. Moves unreleased changelog entries under a new `## <version>` header
5. Runs editorial cleanup on the changelog entries via Claude Code (unless `--no-ai` is passed)
6. Pauses for manual review -- edit `CHANGELOG.md` as needed, then press Enter to continue
7. Creates a `release/<version>` branch, commits, pushes, and opens a PR

After the script completes:
1. Review and squash-merge the PR on GitHub
2. The auto-tag workflow automatically creates the `v<version>` tag and triggers the CD pipeline
3. The CD pipeline builds the DMG and publishes the GitHub Release

If the auto-tag workflow fails, run `./cut-release.sh --tag-only` to manually create and push the tag (reads the version from the `VERSION` file).

## Installer transition and tests

One `install.sh` selects `--app ember|sapphire`, defaulting to Ember; Sapphire currently requires `--nightly`.
New Ember installs use `Devora Ember.app`, retaining `com.devora-org.devora` for macOS TCC continuity.
The installer tries `Devora-Ember.dmg` first, with a temporary fallback to the old `Devora.dmg` stable asset (remove after the first renamed stable release).
A legacy DMG's `Devora.app` is copied under the new directory name; its old display name remains until upgrading to a renamed release.
It removes only the exact legacy `/Applications/Devora.app` after the new app is copied and quarantine is cleared; source `Devora-*.app` installs remain.
Completion generation uses the selected installed app.
`bash scripts/test-install.sh` runs in the root test command, with stubbed downloads/mounts and `DEVORA_APPLICATIONS_DIR` pointing at a temporary directory; it needs neither the network nor access to real installed apps.
It checks new/legacy assets, app/channel selection, completion source, copy/quarantine/download failures, running-app rejection and legacy/source preservation.

## Cost considerations

The **test** job runs on `ubuntu-latest`, which is free for public repos and cheap for private ones.

The **build-and-release** job runs on `macos-15` because the build process requires macOS-specific tools (`hdiutil`, `plutil`, Xcode toolchain).
MacOS runners on GitHub Actions are roughly 10x more expensive than Linux runners.
The test job is deliberately kept on Linux to avoid unnecessary MacOS runner usage -- the build job only runs after tests pass.

## Troubleshooting

### Smoke test fails on version check

The smoke test verifies that `Info.plist` does not contain the `tauri.conf.json` placeholder version `0.1.0`.
This fails if `bundler/macos-ember/populate-app-resources.sh` did not patch the plist correctly.
Check that `VERSION` contains a valid version and that the `plutil -replace` calls in `populate-app-resources.sh` succeeded.

### Nightly release already exists

The pipeline deletes existing nightly releases before creating new ones.
If deletion fails (e.g. due to permissions), the create step will fail.
Verify the `GH_TOKEN` has `contents: write` permission.

### Changelog extraction returns empty notes

The stable release step extracts notes by matching `## <version>` (without the `v` prefix) in `CHANGELOG.md`.
If the section is missing or the heading format differs, notes will be empty and the fallback message is used.
Ensure `cut-release.sh` was used to prepare the release, as it formats the changelog correctly.
