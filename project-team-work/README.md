# Team-Work

A Claude Code plugin that provides a single skill for leading and coordinating a team of agents on a task.

It ships with Devora and is loaded automatically when Claude Code is launched from a Devora session.

## Contents

- `cc-plugin/skills/team-work/` — the `team-work` skill

## Usage

The skill is user-invocable only (`disable-model-invocation: true`): Claude will not auto-invoke it, so it only runs when explicitly requested via `/team-work`.
