#!/usr/bin/env bash
# Checks the module map (ADR-015) against the repository: scripts/ci/modules.lock.json leaves no tracked file unmapped,
# no file an app bundles counts as affecting nothing, and affected.sh selects as the cases below say. A wrong "false"
# would skip a job that must run.
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
lock="$here/modules.lock.json"
failures=0
fail() {
	echo "FAIL: $1" >&2
	failures=$((failures + 1))
}

# The module whose jobs `affected.sh` must (or must not) run for these changed paths together.
check() {
	local module=$1 expected=$2
	shift 2
	local actual
	actual=$(printf '%s\n' "$@" | "$here/affected.sh" "$lock" | sed -n "s/^$module=//p")
	[ "$actual" = "$expected" ] || fail "expected $module=$expected for: $*"
}

# Every tracked file is in `nothing`, `everything` or some module's inputs, so none runs everything by accident.
unmapped=$(git -C "$root" ls-files | jq -r -R -s --slurpfile lock "$lock" '
	$lock[0] as $map
	| def holds($paths): . as $p | any($paths[]; . as $d | if $d | endswith("/") then $p | startswith($d) else $p == $d end);
	split("\n")[] | select(length > 0)
	| select((holds($map.nothing.paths) or (split("/") | last | IN($map.nothing.names[]))
		or holds($map.everything.paths) or (split("/") | last | IN($map.everything.manifests[]))) | not)
	| select(. as $p | [$map.modules[].inputs] | any(.[]; . as $inputs | $p | holds($inputs)) | not)')
[ -z "$unmapped" ] || fail "tracked files in no module, \`everything\` or \`nothing\` (add them to scripts/ci/modules.toml):
$unmapped"

# `nothing` matches any file named CLAUDE.md, so each one is pinned here: a new one that the product or a test reads
# (an agent's instructions template, a fixture) must not skip both apps unnoticed.
agent_docs="CLAUDE.md
acceptance/CLAUDE.md
bundler/CLAUDE.md
project-code-cleanup/CLAUDE.md
project-core/CLAUDE.md
project-core/crates/devora-agent-space/CLAUDE.md
project-core/crates/devora-agent/CLAUDE.md
project-debi/CLAUDE.md
project-detached-flow/CLAUDE.md
project-ember/CLAUDE.md
project-ember/src/ui/layers/CLAUDE.md
project-judgess/CLAUDE.md
project-sapphire/CLAUDE.md
project-sapphire/Packages/TranscriptKit/CLAUDE.md
project-sapphire/Sources/AppShell/AgentHub/CLAUDE.md
project-sapphire/Sources/AppShell/CLAUDE.md
project-sapphire/Sources/AppShell/Preview/CLAUDE.md
project-sapphire/Sources/AppShell/Profiles/CLAUDE.md
project-sapphire/Sources/AppShell/Repos/CLAUDE.md
project-sapphire/Sources/AppShell/Settings/CLAUDE.md
project-sapphire/Sources/AppShell/TaskCreation/CLAUDE.md
project-sapphire/Sources/AppShell/WorkspaceHub/CLAUDE.md
project-sapphire/Sources/Layers/CLAUDE.md
project-sapphire/Sources/Store/CLAUDE.md
project-sapphire/Sources/TerminalKit/CLAUDE.md
project-sapphire/Tests/CLAUDE.md
project-sapphire/UITests/CLAUDE.md
project-sapphire/acceptance/CLAUDE.md
project-team-work/CLAUDE.md
web-renderer/CLAUDE.md"
unpinned=$(git -C "$root" ls-files -- 'CLAUDE.md' '*/CLAUDE.md' | grep -vxF "$agent_docs")
[ -z "$unpinned" ] || fail "new CLAUDE.md files, which CI treats as affecting no app: confirm each is agent documentation
that no build, test or app reads, and add it to agent_docs in scripts/ci/modules.test.sh:
$unpinned"

# checks.toml is in `nothing`, which is right only while no macOS job runs its tasks: not in a ci.yml job but `test`
# (Linux), not from a mise.toml's task or a script those jobs run (any `mise run <task>` without -C falls through to the
# root's), and not as a root task's dependency. Comment lines and docs don't run anything. cd.yml is left out: the
# release runs `mise run test` on macOS, but it isn't a pull request's selection, and it runs everything; so is this
# file, whose fixture below holds such runs on purpose.
# Prints each place under <repo> where a macOS job can reach a checks.toml task. Without mise (the changes job has
# none): each task is a top-level `[name]` table there.
reachable_checks() {
	local repo=$1 checks_tasks task_names runs_check
	checks_tasks=$(sed -nE 's/^\[([A-Za-z0-9_:-]+)\][[:space:]]*$/\1/p' "$repo/checks.toml")
	[ -n "$checks_tasks" ] || { echo "checks.toml: no [task] tables (is it still in the root mise.toml's [task_config] includes?)"; return; }
	task_names=$(printf '%s\n' "$checks_tasks" | paste -sd'|' -)
	runs_check="mise( run| r)?( +--?[a-zA-Z-]+(=[^ ]+)?)* +($task_names)([[:space:]\"';)\`|&]|\$)"
	# shellcheck disable=SC2016 # The `$` in the program are awk's fields, not the shell's.
	(cd "$repo" && git ls-files -z -- '.github/workflows/*.yml' '*mise.toml' '*.sh' '*.ts' '*.mjs' '*.js' '*.py' \
		':!.github/workflows/cd.yml' ':!scripts/ci/modules.test.sh' | xargs -0 awk -v pattern="$runs_check" '
			FNR == 1 { job = "" }
			FILENAME ~ /^\.github\/workflows\// && /^  [a-zA-Z0-9_-]+:[[:space:]]*$/ { job = $1; sub(/:$/, "", job) }
			/^[[:space:]]*(#|\/\/)/ { next }
			$0 ~ pattern && $0 !~ / -C / && !(FILENAME == ".github/workflows/ci.yml" && job == "test") {
				print FILENAME ":" FNR ": " $0
			}')
	grep -nE "^[[:space:]]*depends[[:space:]]*=.*[\"'](${task_names})[\"']" "$repo/mise.toml" | sed 's/^/mise.toml:/'
}
reachable=$(reachable_checks "$root")
[ -z "$reachable" ] || fail "a macOS job can reach a checks.toml task, which a change there wouldn't run (move the
task to the root mise.toml, or run it with -C):
$reachable"

# The same check finds each kind of reach, with this machine's awk (CI's is mawk): a fixture repo with one of each,
# beside the forms that don't reach (the Linux test job, -C, a comment, a task whose name only starts the same).
fixture=$(mktemp -d)
mkdir -p "$fixture/.github/workflows" "$fixture/project-app"
printf '[lint]\nrun = "true"\n' > "$fixture/checks.toml"
printf '[tasks.build]\ndepends = ["lint"]\nrun = "true"\n' > "$fixture/mise.toml"
printf 'jobs:\n  test:\n    steps:\n      - run: mise run lint\n  app:\n    steps:\n      - run: mise run --no-deps lint && true\n' \
	> "$fixture/.github/workflows/ci.yml"
printf '[tasks.check]\nrun = "mise run -C .. lint"\n# mise run lint\n[tasks.lint-swift]\nrun = "mise run lint-swift"\n[tasks.all]\nrun = "mise lint"\n' \
	> "$fixture/project-app/mise.toml"
git -C "$fixture" init -q && git -C "$fixture" add -A
found=$(reachable_checks "$fixture")
expected=".github/workflows/ci.yml:7:       - run: mise run --no-deps lint && true
project-app/mise.toml:7: run = \"mise lint\"
mise.toml:2:depends = [\"lint\"]"
[ "$found" = "$expected" ] || fail "the checks.toml reach check found, in its fixture:
$found
instead of:
$expected"
rm -rf "$fixture"

# What an app bundles must always run its jobs, so a file that starts being bundled can't skip them silently.
# Two exceptions, both in `nothing`:
# - CHANGELOG.md is packed, but neither app reads it at runtime or in a test, and nearly every pull request touches it.
# - A file named exactly CLAUDE.md, inside a directory the bundler builds from (project-core/, project-debi/, ...), is
#   agent documentation: no build, test or app reads it, and no built bundle contains one.
for app in sapphire ember; do
	while IFS= read -r source; do
		[ "$source" = CHANGELOG.md ] && continue
		while IFS= read -r file; do
			[ "$(basename "$file")" = CLAUDE.md ] && continue
			[ "$(printf '%s\n' "$file" | "$here/affected.sh" "$lock" | sed -n "s/^$app=//p")" = true ] ||
				fail "$file is bundled into $app (under $source) but doesn't run its jobs"
		done < <(git -C "$root" ls-files -- "$source")
	done < <(bash "$root/bundler/macos-ember/populate-app-resources.sh" --app "$app" --list-sources)
done

# A build manifest runs every app, since it may change the graph the lock was derived from (and the sapphire job, which
# runs then, checks the lock); so does a path no module holds.
check sapphire true project-ember/package.json
check ember true project-sapphire/Package.swift
check ember true project-core/crates/devora-ffi/Cargo.toml
check sapphire true a-new-top-level-dir/file.txt
check ember true a-new-top-level-file.md

# The checks only the Linux jobs run (checks.toml) run neither app; the root mise.toml, which pins the tools, runs both.
check ember false checks.toml
check sapphire false checks.toml
check ember true mise.toml

# Today's cases, from the classifiers this replaced.
check sapphire true web-renderer/src/index.ts
check sapphire true project-sapphire/Sources/Store/Store.swift
check sapphire true project-core/crates/devora-state/src/store.rs
check sapphire true project-core/Cargo.lock
check sapphire true project-debi/internal/cli/preview.go
check sapphire true project-debi/go.mod
check sapphire true acceptance/features/workspace-hub.feature
check sapphire true project-ember/tests/support/fixture-helper.ts
check sapphire true mise.toml
check sapphire true .github/workflows/ci.yml
check sapphire true themes/default.json
check sapphire true VERSION
check sapphire true USER_GUIDE.md
check sapphire true scripts/ci/affected.sh
check sapphire true some-new-dir/file.txt
check sapphire true project-ember/src/main.ts project-sapphire/App/project.yml
check sapphire true
check sapphire false project-ember/src/main.ts
check sapphire false project-ember/src-tauri/src/lib.rs project-ember/tests/steps/session.steps.ts
check sapphire true project-judgess/src/lib.rs
check sapphire false AGENTS.md CHANGELOG.md docs/adrs/ADR-010-sapphire-ui-stack.md project-core/CLAUDE.md
check sapphire true bundler/macos-ember/bundle-fingerprint.sh
check sapphire true bundler/macos-sapphire/bundle-sapphire.sh
check sapphire true project-status-line/main.go
check sapphire true project-code-cleanup/cc-plugin/plugin.json
check sapphire true base-icon-sapphire.png
check sapphire true scripts/ci/tools/package.json scripts/ci/tools/package-lock.json
check sapphire false base-icon-ember.png
check sapphire false .github/workflows/cd.yml
check ember true project-ember/src/main.ts
check ember true project-ember/src-tauri/src/lib.rs
check ember true project-core/crates/devora-state/src/store.rs
check ember true project-core/crates/devora-ffi/src/lib.rs
check ember true project-core/Cargo.lock
check ember true web-renderer/src/index.ts
check ember true acceptance/features/workspace-hub.feature
check ember true bundler/macos-ember/bundle-ember.sh
check ember true project-debi/go.mod
check ember true project-judgess/src/lib.rs
check ember true project-status-line/main.go
check ember true project-code-cleanup/cc-plugin/plugin.json
check ember true themes/catppuccin-macchiato.conf
check ember true USER_GUIDE.md
check ember true base-icon-ember.png
check ember true mise.toml
check ember true .github/workflows/ci.yml
check ember true scripts/ci/tools/package.json scripts/ci/tools/package-lock.json
check ember true scripts/ci/affected.sh
# A CLAUDE.md is agent documentation, wherever it is: it runs neither app.
check ember false project-core/CLAUDE.md
check sapphire false project-core/CLAUDE.md
check ember true some-new-dir/file.txt
check ember true project-sapphire/Sources/Store/Store.swift project-ember/src/main.ts
check ember true
check ember false project-sapphire/Sources/Store/Store.swift
check ember false project-sapphire/App/project.yml CHANGELOG.md
check ember false docs/adrs/ADR-014-native-agent-transcript.md CLAUDE.md AGENTS.md README.md
check ember false base-icon-sapphire.png
check ember false .github/workflows/cd.yml

if [ "$failures" -gt 0 ]; then
	echo "modules: $failures check(s) failed" >&2
	exit 1
fi
echo "modules: all checks passed"
