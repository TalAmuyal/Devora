# Judgess

Judgess is the from-scratch successor to [Judge](../project-judge/), reducing Claude Code permission fatigue by auto-deciding permission requests.
Judgess is implemented as a subcommand of Devora's CLI (`debi judgess`, in `project-debi/internal/judgess/`).

This folder holds only the Claude Code plugin packaging; all logic lives in `project-debi`.

## Hook

`cc-plugin/hooks/hooks.json` registers a `PermissionRequest` hook that runs `debi judgess`, matching every request except `ExitPlanMode` (excluded to keep crit's plan-exit integration untouched).
In the bundled app, `${CLAUDE_PLUGIN_ROOT}` is `Resources/cc-plugins/judgess`, so `../../bundled-apps/debi` resolves to the bundled `debi` binary.

`debi judgess` reads the request as JSON on stdin and reports its decision via the process exit code and stdout.
