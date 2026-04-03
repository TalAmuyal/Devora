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

- Add health command to Debi (`debi health`) for checking Devora dependencies
- `-h`/`--help` flag support in the debi CLI
- 23 git shortcut commands (`gaa`, `gaac`, `gaacp`, `gaaa`, `gaaap`, `gb`, `gbd`, `gbdc`, `gcl`, `gcom`, `gd`, `gfo`, `gg`, `gl`, `gpo`, `gpof`, `gpop`, `gri`, `grl`, `grlp`, `grom`, `gst`, `gstash`) for common git workflows
- `PassthroughError` and `RunPassthrough` in the process package for terminal-connected command execution
- Enhanced redaction for Judge test cases: path segment hashing, git hash replacement, commit message/PR title redaction, timestamp zeroing, description removal, and audit mode (`--audit`)

### Fixed

- Footer now stays attached to the bottom of the screen even when content is short

### Changed

- Include version in DMG filename (e.g., `Devora_2026-03-28.0.dmg`)

### Added

- Changelog file to track future changes between releases
- Version tracking (displayed in the settings page)
