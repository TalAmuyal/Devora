# Extract Devora's Non-UI Backend into a UI-Agnostic Core Crate

**Status**: proposed, not started.
**Type**: refactor — no user-visible behavior change.
**Context**: [macOS-native UI thought experiment](macos-native-ui-vision.html) (phase 0 of its migration path). This ticket is worth doing regardless of whether a native UI is ever built.

## Problem

Devora's domain logic — profiles, workspaces, task creation, repo cloning, credentials, and health checks — lives inside the Tauri app crate (`project-ember/src-tauri/`), mixed with code that only exists to serve the webview (Tauri commands, PTY plumbing, the IPC server, theming, the test harness).

Some domain modules also depend on Tauri types directly, e.g., progress is reported through Tauri's IPC channel type, and bundled resources are located through the Tauri app handle.

As a result:
- The domain logic can only be used, built, and tested as part of the Tauri app
- The boundary between "what Devora does" and "how Ember shows it" is implicit, so it erodes with every feature
- Any future front end (a native UI, a CLI surface, a headless test driver) would have to either depend on Tauri or duplicate the logic

## Goal

A standalone Rust library crate (working name `devora-core`) that owns Devora's domain logic, has **no dependency on Tauri or any other UI framework**, and that Ember consumes as an ordinary dependency.

After this change, Ember's Rust side is a thin adapter: it translates Tauri commands and events into calls on the core and back.

## Scope

Move everything that answers "what does Devora do?" into the core.
At the time of writing that means roughly the profile, workspace, workspace-creation, repo-clone, credentials, and health modules, but **classify by responsibility, not by file name**.
Modules may have been split, merged, or added since this was written.

A rule of thumb for any given piece of code:
- **Core**: it would still be needed if Devora's UI were rewritten from scratch (reading/writing profiles and config, discovering and creating workspaces and worktrees, running the prepare command, keychain access, dependency checks)
- **Ember**: it only exists because the UI is a webview (Tauri command signatures, the ACL, PTY-to-xterm.js streaming, theme delivery to CSS, the eval bridge)
- **Undecided**: the IPC server that external tools (Crit, `debi preview`, Judge) talk to. Its *protocol* is product-level and should be documented as such (see below). Where its *implementation* lives is a judgment call for whoever does the work. Moving it into the core is reasonable if it can be done without UI coupling, and fine to defer otherwise.

### Decoupling the seams

Where a domain module currently reaches into Tauri, replace the Tauri type with a UI-agnostic abstraction owned by the core, for example:
- **Progress/event reporting** (today a Tauri channel): a plain callback or event-sink trait. The event *types* (step, log line, done, failed, cancelled) stay as they are, since they are already serializable domain types
- **Locating bundled resources** (today via the app handle): pass the resolved paths in explicitly, or have the caller provide a small "environment" value
- **Cancellation**: keep it as a core concept that does not depend on how the UI triggers it

Ember then adapts these: it wraps a Tauri channel in the core's event sink, resolves resource paths, and passes them in.

### Documenting the external IPC contract

As part of this ticket, write down the IPC protocol that tools outside the app rely on: the environment variables a session exposes, the routes, and the request/response shapes.
Put it next to whichever code owns the server.

That contract is what keeps Judge, Crit, and Debi working across UI changes, so it should be a reviewed artifact rather than something you have to infer from the server code.

## Non-goals

- **No behavior change.** Users must not be able to tell this happened
- **No new front end and no FFI bindings.** Exposing the core to Swift (UniFFI) or anything else is a separate decision for later. The core should simply not make that hard (e.g., avoid leaking Tauri or webview-specific types through its public API)
- **No merge with Debi.** Debi is Go and has some overlapping concerns (config reading, workspace awareness). Unifying them is out of scope; if overlaps are noticed, list them in the PR for a future ticket
- **No API redesign beyond what decoupling needs.** Keep the existing function shapes where possible, to keep the diff reviewable and to minimize conflicts with parallel work

## Coordination with parallel work

This touches files other features are likely to be changing.
To that end, all other work will halt until this ticket is done (as a single multi-commit PR), and the following rules apply:
- Prefer **moving code first, then decoupling** in separate commits, so reviewers can tell a pure move from a semantic change
- Land it in **small slices** (one domain area at a time) instead of one big-bang commit. Each commit in the PR should leave `master` green
- Any domain module added after this ticket was written belongs in the core too, as long as it fits the rule of thumb above
- Once the crate exists, new domain logic should land in the core by default. Add a short note on that boundary to the relevant `CLAUDE.md` so it holds

## Acceptance criteria

- A core crate exists in the repo, with no Tauri (or other UI-framework) dependency in its manifest
- The core's unit tests run on their own, without building the Tauri app, and have a mise task
- Ember depends on the core. Its Rust side contains no domain logic beyond adapting the core to Tauri
- All existing Rust tests pass, and the Ember acceptance suite (`mise test-e2e`) passes unchanged, with no feature file edits needed
- The external IPC contract is documented
- The bundler and the build fingerprint (`bundler/macos-ember/`) account for the new crate, so stale bundles are still detected

## Notes

- Crate location and name: a top-level `project-*` directory - A top-level directory signals that the core isn't Ember-specific

## Open questions

- Should the IPC server move into the core now, or wait until a second consumer exists?
