#!/usr/bin/env bash
# Prints "true" when the changed paths on stdin (one per line) may affect Devora Sapphire's build or tests, else "false".
# It lists what can't affect Sapphire and treats everything else as relevant, so a path nobody thought of makes the job run; no paths also counts as relevant.
set -uo pipefail

irrelevant() {
	case "$1" in
		# Sapphire's acceptance tests import Ember's fixture helpers.
		project-ember/tests/support/*) return 1 ;;
		project-ember/*) return 0 ;;
		project-debi/* | project-judgess/* | project-status-line/* | project-code-cleanup/* | project-detached-flow/* | project-team-work/*) return 0 ;;
		bundler/macos-ember/* | bundler/hdiutil-create-retry.sh | bundler/CLAUDE.md | bundler/docs/*) return 0 ;;
		docs/* | scripts/hooks/*) return 0 ;;
		.github/workflows/cd.yml | .github/workflows/changelog.yml | .github/workflows/auto-tag.yml) return 0 ;;
		CHANGELOG.md | README.md | CONTRIBUTING.md | SECURITY.md | LICENSE | THIRD_PARTY_LICENSES.md | install.sh | cut-release.sh | base-icon-ember.png | additional-instructions.md) return 0 ;;
		CLAUDE.md | */CLAUDE.md) return 0 ;;
		*) return 1 ;;
	esac
}

any=false
while IFS= read -r path; do
	[ -n "$path" ] || continue
	any=true
	if ! irrelevant "$path"; then
		echo true
		exit 0
	fi
done
if [ "$any" = true ]; then echo false; else echo true; fi
