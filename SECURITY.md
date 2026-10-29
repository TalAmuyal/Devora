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

- **Judgess auto-approval rules** -- Vulnerabilities in the Judgess component (`project-judgess/`) that could allow unintended command execution or bypass approval checks.
- **Shell script injection or path traversal** -- Issues in Devora's own shell scripts (`install.sh`, `bundler/macos-ember/bundle-ember.sh`, etc.) that could lead to arbitrary code execution, path traversal, or privilege escalation.

## Out of scope

The following should be reported to their respective upstream projects, not to Devora:

- **Claude Code** -- Security issues in Claude Code itself should be reported to [Anthropic](https://www.anthropic.com/).
