# ADR-006: Two Builds Side by Side (Ember and Sapphire)

## Status

Accepted

## Date

2026-10-28

## Context

Devora was built for macOS, and its only build, Ember, is Tauri + TypeScript + xterm.js.
Two goals pull it in different directions:

- **Reach:** a build that also runs on Linux.
	Tauri already gives Ember that path.
- **Depth:** a build that uses the newest macOS APIs throughout, embeds agents natively instead of hosting Claude Code in a terminal, and survives crashes without losing anything.
	A web stack caps how far that can go.

One build cannot be both the portable baseline and the hyper-optimized native app without compromising each.

The plan and discussion live in TalAmuyal/Devora#204.

## Decision

- **Two builds from one monorepo, shipped side by side for the foreseeable future**
	- **Devora Ember** (`project-ember/`): the general Mac/Linux build. It keeps its name internally and is published as `Devora Ember.app`.
	- **Devora Sapphire** (`project-sapphire/`): a native macOS-only build (see ADR-010), published as `Devora Sapphire.app`.
- **Ember is renamed on output, without migration tooling.**
	The build produces `Devora Ember.app`, and the change is announced in the CHANGELOG highlights.
- **The core enables, Ember opts in.**
	Every new feature is designed in the shared core first (ADR-007, ADR-008).
	Sapphire implements it first, and Ember adopts it when that is worth doing.
	Ember is not frozen, and strict parity is not a goal.
- **Sapphire starts at Ember parity.**
	Its first milestone is Ember's surface; the direction after that is set by using it daily.
- **Ember on Linux is a separate track, but this work must not make it harder.**
	No macOS-only API enters the shared core without a trait seam.

## Alternatives considered

- **One app with two modes.** Rejected: it couples release cadence and risk, and drags the web stack into the native build.
- **Replace Ember with Sapphire.** Rejected: it gives up the Linux path and the working product during a multi-phase rewrite.
- **Ember frozen as a baseline.** Rejected: shared-core features are cheap for Ember to adopt, and freezing it would let it rot.

## Consequences

- Two apps to build, release and QA.
	Most coverage moves into shared-core tests, and each app's UI acceptance tests stay focused on its own surface.
- The bundler, `install.sh` and the release workflow learn a second app name.
- Each app keeps its own state databases (ADR-008), so running both at once is safe.
