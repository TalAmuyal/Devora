# CD Pipeline

## Overview

The CD pipeline (`.github/workflows/cd.yml`) automates building and publishing Devora releases.
It produces a macOS DMG containing the full app bundle and publishes it as a GitHub Release.

There are two release types: **nightly** builds that run daily, and **stable** releases triggered by version tags.

## Triggers

| Trigger | Schedule / Pattern | Release type |
|---|---|---|
| Cron | Daily at 03:00 UTC (midnight UTC-3) | Nightly |
| Tag push | `v*` (e.g. `v2026-03-28.0`) | Stable |

## Build process

The pipeline has two jobs:

1. **test** -- runs on `ubuntu-latest`. Identical to the CI workflow: checks out the repo, sets up mise, and runs `mise run test`.
2. **build-and-release** -- runs on `macos-15`, gated on the test job passing.

### Build steps (build-and-release)

1. **Checkout** with `fetch-depth: 0` (full history, needed for version computation).
2. **Set up mise** for Go, Python, and other tool versions.
3. **Restore cached dependencies** (see [Caching](#caching) below).
4. **Download third-party dependencies** via `./bundler/download-deps.sh`. If the cache was hit, this is a no-op since the script skips already-present files.
5. **Build DMG** via `./bundler/macos/bundle.sh --dmg`. This compiles Debi, Status-line, bundles all resources, and creates a compressed DMG.
6. **Smoke test** -- verifies that:
   - The DMG file was created
   - Bundled binaries (`debi`, `uv`, `glow`) are arm64
   - `Info.plist` is valid XML
   - The version in `Info.plist` was patched (not the placeholder `1.0`)
7. **Publish release** (see [Release types](#release-types)).

## Release types

### Nightly

Nightly builds run on the daily cron schedule.
They produce two GitHub Releases:

- **Dated release** (`nightly-YYYY-MM-DD`) -- a snapshot for that specific day. If re-run on the same day, the existing release is deleted and recreated.
- **Rolling alias** (`nightly-latest`) -- always points to the most recent nightly. Deleted and recreated on each run.

Both are marked as **pre-release** and have `latest=false` so they never appear as the "Latest" release on GitHub.

Release notes include the short commit SHA and date, with a note that the build may be unstable.

### Stable

Stable releases are triggered by pushing a tag matching `v*` (e.g. `v2026-03-28.0`).

The publish step extracts release notes from `CHANGELOG.md` by finding the section header matching the version (without the `v` prefix) and collecting content up to the next `##` header.
If no matching section is found, it falls back to a generic "Release VERSION" message.

Stable releases are **not** marked as pre-release and use the default `latest` behavior (the most recent stable release is shown as "Latest" on GitHub).

## How to cut a stable release

Use the `cut-release.sh` script at the repo root:

```bash
./cut-release.sh          # auto-generates a date-based version (YYYY-MM-DD.N)
./cut-release.sh 1.2.3    # or provide an explicit version
```

The script:
1. Reads the current version from `VERSION`
2. Generates a new version (date-based by default, with an incrementing patch number for same-day releases)
3. Validates that the `## Unreleased` section in `CHANGELOG.md` has content
4. Moves unreleased changelog entries under a new `## <version>` header
5. Writes the new version to `VERSION`

After running the script, review the changes, commit, tag, and push:

```bash
git add VERSION CHANGELOG.md
git commit -m 'Release <version>'
git tag v<version>
git push origin master --tags
```

The tag push triggers the CD pipeline, which builds and publishes the stable release.

## Dependency management

Third-party dependencies are declared in `bundler/3rd-party-deps.json`.
Each entry specifies:

- `name`, `version`, `license`, `source_repo` -- metadata
- `download_url` -- direct download link for the release artifact
- `sha256` -- SHA-256 checksum of the downloaded archive
- `archive_type` -- `tar.gz` or `dmg`
- `extract_path` -- file or directory to extract from the archive

The `bundler/download-deps.sh` script processes this manifest:
1. Skips dependencies that are already present in `bundler/macos/3rd-party-apps/`
2. Downloads the archive
3. Verifies the SHA-256 checksum
4. Extracts the specified file or directory

To update a dependency, edit `3rd-party-deps.json` with the new version, download URL, and checksum.
The cache key will change automatically, triggering a fresh download on the next pipeline run.

### Caching

The pipeline caches `bundler/macos/3rd-party-apps/` using a key derived from the hashes of both `bundler/3rd-party-deps.json` and `bundler/download-deps.sh`.
This means the cache is invalidated when either the dependency manifest or the download script changes.

## Cost considerations

The **test** job runs on `ubuntu-latest`, which is free for public repos and cheap for private ones.

The **build-and-release** job runs on `macos-15` because the build process requires macOS-specific tools (`hdiutil`, `plutil`, Xcode toolchain).
MacOS runners on GitHub Actions are roughly 10x more expensive than Linux runners.
The test job is deliberately kept on Linux to avoid unnecessary MacOS runner usage -- the build job only runs after tests pass.

## Troubleshooting

### Cache miss causes slow builds

If `3rd-party-deps.json` or `download-deps.sh` changed, the cache key no longer matches and all dependencies are re-downloaded.
This is expected.
Builds without cache take longer due to downloading Kitty (~80 MB DMG) and other dependencies.

### Smoke test fails on version check

The smoke test verifies that `Info.plist` does not contain the placeholder version `1.0`.
This fails if `bundle.sh` did not patch the plist correctly.
Check that `VERSION` contains a valid version and that the `sed` replacement in `bundle.sh` matches the plist template.

### Nightly release already exists

The pipeline deletes existing nightly releases before creating new ones.
If deletion fails (e.g. due to permissions), the create step will fail.
Verify the `GH_TOKEN` has `contents: write` permission.

### Changelog extraction returns empty notes

The stable release step extracts notes by matching `## <version>` (without the `v` prefix) in `CHANGELOG.md`.
If the section is missing or the heading format differs, notes will be empty and the fallback message is used.
Ensure `cut-release.sh` was used to prepare the release, as it formats the changelog correctly.

### Checksum mismatch during dependency download

If a dependency's download URL now points to a different file (e.g. the upstream project re-tagged a release), the SHA-256 check will fail.
Download the new artifact manually, compute its checksum (`shasum -a 256 <file>`), and update `3rd-party-deps.json`.
