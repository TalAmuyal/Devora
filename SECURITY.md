# Security Policy

## Supported versions

Devora is a young project. Only the latest release is supported with security updates.

| Version        | Supported |
| -------------- | --------- |
| Latest release | Yes       |
| Older releases | No        |

## Reporting a vulnerability

Please **do not** open a public issue for security vulnerabilities.

Instead, use GitHub's private security advisory feature to report the issue confidentially:

1. Go to [New Security Advisory](https://github.com/TalAmuyal/Devora/security/advisories/new).
2. Fill in the details and submit the draft advisory.

You should expect an initial response within a few days. The advisory will remain private until a fix is available.

## Scope

The following are considered security issues in Devora:

- **Judge auto-approval rules** -- Vulnerabilities in the Judge component (`project-judge/`) that could allow unintended command execution or bypass approval checks.
- **Shell script injection or path traversal** -- Issues in Devora's own shell scripts (`ccc.sh`, `bundler/macos/bootstrap.sh`, `bundler/macos/bundle.sh`, etc.) that could lead to arbitrary code execution, path traversal, or privilege escalation.
- **Bundled third-party binaries** -- If you discover a vulnerability in a binary bundled with Devora (Kitty, uv, glow), please report it to the upstream project first, then notify Devora so the bundled version can be updated.

## Out of scope

The following should be reported to their respective upstream projects, not to Devora:

- **Claude Code** -- Security issues in Claude Code itself should be reported to [Anthropic](https://www.anthropic.com/).
- **Kitty, uv, glow** -- Vulnerabilities in these tools that are not specific to how Devora bundles or invokes them should be reported to their respective maintainers.
