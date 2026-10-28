# ADR-007: Shared Rust Core and Debi in Rust

## Status

Accepted

## Date

2026-10-28

## Context

Devora's non-UI logic is split across three places:

- Ember's Rust backend (`project-ember/src-tauri/`): workspaces, worktree creation, profiles, credentials, PTY and theme, about 6.6k lines.
- Debi (`project-debi/`, Go): an overlapping implementation of workspaces, profiles, credentials and health, plus PR and task-tracker commands.
	Ember shells out to it for health checks.
- `ccc.sh`: assembles Claude Code's flags and environment (plugin dirs, effort, permission mode, telemetry variables).

Ember and Debi have to agree on shared contracts by convention, for example the keychain item format.
A second UI (Sapphire, ADR-006) would make that a third copy.

## Decision

- **A new Cargo workspace, `project-core/`, owns every non-UI behavior**
	- It is split into deep modules: crates with a small, stable interface over a large hidden implementation.
		The initial split is indicative (config, workspace, git, platform, health, pty, agent, state, nvim, theme, ffi) and is settled while extracting.
	- **One facade per UI boundary.**
		Each UI binds a single `DevoraCore` object (Ember through Tauri commands, Sapphire through UniFFI) and never reaches into individual crates.
	- **Commands in, events out.**
		Core state changes are published as typed events, and UIs render from them.
	- **Platform seams are traits** (keychain, login-shell environment, process spawning, file watching), with macOS implementations, so Linux only adds implementations.
	- **In-process first.**
		Agent and session management are libraries, not services, so a future detach or daemon project can host the same crates.
- **Prefactor before Sapphire**
	- Ember's non-UI Rust moves into the core area by area, with no behavior change.
	- The gate is Ember's full acceptance suite, run on a Mac.
- **Debi is rewritten in Rust over the core, by command group**
	- `ccc` goes first: `ccc.sh` becomes `debi ccc`, built on a `LaunchSpec` from the core, and ships with a `ccc` alias, so Ember's terminal, the `ccc` command and Sapphire's agent sessions all launch Claude identically.
	- Each group keeps its CLI surface byte-compatible (flags, `--json` output, exit codes), guarded by golden tests recorded from the Go CLI before porting.
	- Where Ember and Debi overlap, the core crate becomes the single implementation.
		Two implementations of one behavior never stay alive.
	- Go is removed from the toolchain once the last group lands.
- **Judgess stays its own project** (`project-judgess/`), shipped both as a library and as a plugin binary, so the core can link it (ADR-009).

## Alternatives considered

- **Extract only Ember's backend and leave Debi in Go.** Rejected: it keeps the Go/Rust duplication, and a second UI would make it a third copy.
- **A thin Go shell over the core.** Rejected: two toolchains with no remaining benefit.
- **Keep `ccc.sh` and feed it flags from Debi.** Rejected: launch logic would stay split between bash and Rust, and Sapphire needs it in-process anyway.

## Consequences

- Phase 1 of TalAmuyal/Devora#204 (TalAmuyal/Devora#209) is a pure refactor of Ember. Phase 2 grows the core, and the Debi port runs alongside.
- New non-UI logic lands in `project-core/` from the first extraction onward, so the prefactor doesn't chase a moving target.
- Cross-tool contracts survive unchanged: the keychain convention (`devora-<provider>`, `$USER`), config file formats, and the Detached-Flow `cc-plugin/bin` aliases.
