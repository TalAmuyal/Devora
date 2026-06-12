# Crit Integration

Devora's integration with [Crit](https://crit.md) — the inline-comment tool used for reviewing
plans and code changes.

It ships with Devora; the `cc-plugin` is bundled into `Resources/cc-plugins/crit-integration/`
and loaded automatically (via Claude Code's `--plugin-dir`) when Claude Code is launched from a
Devora session.

## Contents

- `bin/crit` — an Ember/OG-aware wrapper around the bundled `original-crit`. Non-UI subcommands
  (`comment`, `share`, `pull`, `push`, `plan-hook`, …) pass straight through; for UI commands it
  suppresses the browser and drives Crit through Devora's overlay (Ember IPC when `DEVORA_EMBER=1`,
  otherwise a kitty overlay/tab in OG).
- `cc-plugin/skills/devora-plan/` — the `devora-plan` skill: guidance for authoring expressive
  plans with embedded HTML (and Mermaid diagrams) styled with Crit's `--crit-*` design tokens so
  they render seamlessly in Crit's plan view. User-invocable only (`/devora-plan`).
- `exit-plan-test-prompt.md` — a manual test prompt exercising the exit-plan (`plan-hook`) flow.

## Bundling

Only the Ember bundler (`bundler/macos-ember/populate-app-resources.sh`) ships the `cc-plugin`
(as `crit-integration`). The OG bundler is intentionally left untouched as Devora moves from OG
to Ember.
