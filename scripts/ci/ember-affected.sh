#!/usr/bin/env bash
# Prints "true" when the changed paths on stdin (one per line) may affect Devora Ember's build or tests, else "false".
# It lists what can't affect Ember and treats everything else as relevant, so a path nobody thought of makes the jobs run; no paths also counts as relevant.
# What Ember bundles (bundler/macos-ember/populate-app-resources.sh --app ember --list-sources) is never listed here, except CHANGELOG.md (see below); ember-affected.test.sh checks that.
set -uo pipefail

irrelevant() {
	case "$1" in
		project-sapphire/* | base-icon-sapphire.png) return 0 ;;
		# Bundled, but nothing in Ember reads it at runtime or in a test, and nearly every pull request touches it.
		CHANGELOG.md) return 0 ;;
		docs/* | scripts/hooks/*) return 0 ;;
		.github/workflows/cd.yml | .github/workflows/changelog.yml | .github/workflows/auto-tag.yml) return 0 ;;
		CLAUDE.md | AGENTS.md | README.md | CONTRIBUTING.md | SECURITY.md | LICENSE | THIRD_PARTY_LICENSES.md | cut-release.sh | additional-instructions.md) return 0 ;;
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
