# CI tools

`package.json` pins the tools CI installs, so that a new release can't change CI under an unchanged tree, and so that Dependabot can see the pins (it can't read a version inside a workflow).
Today that's the Claude Code CLI (`@anthropic-ai/claude-code`), exact, which the real-Claude acceptance scenarios and the agent CLI tests run.
It lives here, not under `.github/`: GitHub refuses Dependabot changes to files under `.github/` for every ecosystem but github-actions (its pull request is rejected as "invalid or unauthorized changes").

Every reader takes the version from here:

- `.github/workflows/ci.yml`'s two "Install Claude Code CLI" steps (`e2e-ember` and `sapphire`), which install it globally, where the apps' terminals find `claude`.
- `project-sapphire/scripts/test-agent-cli.ts`, which installs it into `project-sapphire/build/agent-cli/<version>` for the agent CLI tests.
- `project-core/crates/devora-agent/tests/session_cli.rs`'s `check_pin`, which checks that the CLI under test is this version.

## Bumps

Dependabot (`.github/dependabot.yml`) checks weekly and opens at most one PR, for this directory only, labelled `skip-changelog`.
Each bump is a PR of its own, never batched with other work, so its CI is about the new version alone.
Its paths count as affecting both apps (`scripts/ci/sapphire-affected.sh`, `scripts/ci/ember-affected.sh`), so the bump runs both apps' CI, about 47 macOS minutes.

When a bump's CI is green, it's merged as it is.

When it's red, the PR becomes an item, rather than sitting open:

1. Tell a break from a flake. Run `mise run flake-report --test '<the failing test>'`: a test that fails on master too is a flake with its own item, and a re-run of the failed jobs shows whether the failure repeats. A failure that repeats, on a test that's green on master, is the new version.
2. For a break, fix what the new version needs on the bump's branch, or report it upstream and keep the old pin.
   The agent CLI tests replay recordings of one CLI version (`project-core/crates/devora-agent/tests/fixtures/claude-<version>/`); when the new version changes the protocol, re-record them as that directory's README describes.
3. Close a bump that can't land yet, saying why on its item, so the next week's bump isn't hidden behind it.
