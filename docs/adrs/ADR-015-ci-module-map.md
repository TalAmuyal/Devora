# ADR-015: One Module Map for CI Test Selection

## Status

Accepted.

## Date

2026-11-20

## Context

A pull request's CI decides which macOS jobs to run with two hand-kept deny-lists, `scripts/ci/sapphire-affected.sh` and `scripts/ci/ember-affected.sh`: each lists the paths that can't affect its app, and anything else runs the app's whole job.
That has served one purpose, skipping a whole app, but it can't select within an app, and its correctness rests on remembering to update the lists.

What CI should know instead is, for each testable unit, which files its result depends on: its own sources, its dependencies' and shared inputs (toolchain pins, lockfiles, fixtures, CI configuration).
Most of that is already written down in the build graphs: SwiftPM's targets and dependencies, Cargo's crates and path dependencies, and the mise tasks' `sources` and `depends`.
What they can't see crosses toolchains: Sapphire's `DevoraCoreFFI` is built from the Rust core, `GhosttyKit` from a commit pinned in `project-sapphire/mise.toml`, the test tools from `project-debi` and `project-core`, and both apps run the scenarios in `acceptance/`.

The job that selects runs on Linux, where SwiftPM isn't available, so the graphs can't be read at selection time.

## Decision

### Modules, declared once

`scripts/ci/modules.toml` declares the testable units, each with its own paths and how its tests run:

- a module per SwiftPM target with tests (Sapphire's `Store`, `TerminalKit`, `Layers`, `AppShell`, TranscriptKit's), per Cargo crate, for the Ember app, Debi (Go), Judgess and the web renderer, and later per UI-test class;
- `deps`: only the edges no build graph shows (the cross-toolchain ones above);
- `inputs`: only the shared files nothing else lists (fixtures, the pins in each `mise.toml`).

What an app loads at runtime is derived too, not declared: the bundled tools (Debi, Judgess), the web renderer's assets, the vendored configurations and the Claude Code pin are what the bundler packs, so `populate-app-resources.sh --list-sources` is part of each app's acceptance, UI and bundle modules' inputs.

Two repo-wide groups complete it: `everything`, whose change runs every module (CI's workflows, the root `mise.toml`, `scripts/ci/` itself, `scripts/ci/tools/`), and `nothing`, which affects no test (`docs/`, the changelog, the READMEs: today's deny-lists).

### Derived, committed, checked

`mise run ci-modules` reads the build graphs (`swift package dump-package` for each package, `cargo metadata` for each workspace, the mise tasks' `sources` and `depends`) and the bundler's list, together with `modules.toml`, and writes `scripts/ci/modules.lock.json`: for each module, every path its result depends on, its dependencies' included.
The paths are directory prefixes (ending in `/`) and exact files, no general globs, so bash matches them with a prefix test; and they're sorted, one per line, so two pull requests rarely conflict in it, and a conflict is resolved by running `ci-modules` again.
It's committed, because the Linux selection job can't derive it.
It runs where the toolchains are (a developer's Mac, and the macOS jobs).

A change to any build manifest (`Package.swift`, `Package.resolved`, `Cargo.toml`, `Cargo.lock`, `package.json`, a lockfile, `go.mod`, a `mise.toml`) runs everything, since it may change the graph.
That full run includes `mise run ci-modules --check`, which fails when the committed lock differs from what it derives, so a stale lock can't outlive the change that made it stale.
It runs in the `sapphire` job, which has the Swift and Rust toolchains and runs whenever everything does, on pull requests and on every push to master.

### Selection

`scripts/ci/affected.sh` (bash and jq, as today's classifiers) takes the changed paths and the lock, and prints the affected modules and the jobs they need.
It is conservative by rule: a path in `everything`, or matching no module and not in `nothing`, runs everything; a push to master runs everything, as now.

### No file left unmapped

`scripts/ci/modules.test.sh`, in the root `mise run test`, fails naming every tracked file that is in no module's inputs, `everything` or `nothing`.
It keeps the invariant `ember-affected.test.sh` checks today: nothing the bundler packs (`populate-app-resources.sh --list-sources`) is in `nothing`.
Today's classifier test cases carry over as selection cases.

### Adoption

1. Replace both classifiers: `affected.sh` prints the same two answers (Sapphire's job, Ember's jobs), and every existing case still holds.
2. Select within Sapphire: TranscriptKit's tests and each Swift target's on their own, and Rust's per crate.
3. Later: reuse a module's green result when the hash of its inputs is one CI has already tested.

## Consequences

- A new file under a module's path is mapped with no edit; a new dependency is picked up from the build graph.
- CI can run part of an app's tests, rather than all or none.
- A committed, derived file must stay fresh; the manifest rule and the `--check` keep it so, at the cost of a full run on every manifest change.
- The hand-declared edges can rot. A missing one makes a pull request skip tests it needed; master's full run still catches the failure, after the merge rather than before it.
  A master run that fails in a module the merged pull request's selection skipped is how a missing edge shows itself, so it's treated like a new flake: an item at the top of Ready, to add the edge (CI can point it out by comparing the two runs; a follow-up).
