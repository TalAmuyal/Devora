# Changelog

All notable changes to Devora will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

Types of changes:
- `Added` for new features
- `Changed` for changes in existing functionality
- `Deprecated` for soon-to-be removed features
- `Removed` for now removed features
- `Fixed` for any bug fixes
- `Security` in case of vulnerabilities

## Unreleased

### Added

- SECURITY.md with vulnerability reporting instructions and scope definition
- CONTRIBUTING.md with development setup and PR guidelines
- uv license file (`uv-license.txt`) bundled into DMG for Apache-2.0 compliance
- Startup error page: `debi workspace-ui` now shows a diagnostic page when Devora's bundled tools are missing from PATH
- MIT LICENSE file
- THIRD_PARTY_LICENSES.md documenting bundled tools and their licenses
- `bundler/3rd-party-deps.json` as source of truth for bundled third-party binaries
- `build` and `test` mise tasks to root `mise.toml`
- Kitty license file (`kitty-license.txt`) bundled into DMG for GPL-3.0 compliance
- Prerequisites, getting started, supported platform, Anthropic disclaimer, and license sections to README.md
- `mac-install` mise task for building and installing Devora.app to /Applications
- Health command to Debi (`debi health`) for checking Devora dependencies
- GitHub credential checking to `debi health` with `gh` as optional dependency
- `-h`/`--help` flag support in the debi CLI
- 23 git shortcut commands (`gaa`, `gaac`, `gaacp`, `gaaa`, `gaaap`, `gb`, `gbd`, `gbdc`, `gcl`, `gcom`, `gd`, `gfo`, `gg`, `gl`, `gpo`, `gpof`, `gpop`, `gri`, `grl`, `grlp`, `grom`, `gst`, `gstash`) for common git workflows
- `PassthroughError` and `RunPassthrough` in the process package for terminal-connected command execution
- Enhanced redaction for Judge test cases: path segment hashing, git hash replacement, commit message/PR title redaction, timestamp zeroing, description removal, and audit mode (`--audit`)
- Changelog file to track future changes between releases
- Version tracking (displayed in the settings page)

### Changed

- Replaced hardcoded personal data in health tests and spec with generic test values
- Updated bundler README to reflect `kitty-license.txt` in app structure
- `debi health --strict` now treats credential failures as missing optional dependencies (exit code 1)
- Include version in DMG filename (e.g., `Devora_2026-03-28.0.dmg`)

### Removed

- `.claude/settings.local.json` from version control (added to `.gitignore`)
- Legacy `cc-simple-statusline.sh` (superseded by Go binary in project-status-line)
- `project-judge/debug.sh`
- "Backlog" sections from project-judge README and project-debi docs
- `jq` as an optional dependency from health check
- Dead `BACKLOG.md` references from project-debi docs

### Fixed

- Use full path for `debi` in macOS bootstrap script to prevent PATH manipulation by shell startup files from blocking launch
- Broken placeholder links in README.md
- Footer now stays attached to the bottom of the screen even when content is short
