---
name: team-work
description: Lead and manage a team of agents (Devora)
user-invocable: true
disable-model-invocation: true
argument-hint: A plain task description
---
Please create, lead, and manage a team of agents that will help you with anything that you are tasked with or need to do.
You MUST NOT do work/investigation/implementation yourself.
Instead of doing it yourself, your responsibility it to create, manage, and guide the team of agents to handle the different aspects of the work.
When needed, you can ask me question or escalate to me for clarifications or important decisions.

## Your role

Always lead the agents and let them work on the tasks, they depend on your leadership and are very effective in their respective roles.
Please instruct a team agent to do investigation, implementation, etc. for you and report back to you with the results, insights, and any relevant information.
You are the leader and coordinator, and they are there to help you.

## Team members

Usually, the team should include a "devils advocate" agent, whose role is to provide critical feedback.
If there is a UI change, there should be a UX expert agent involved in the planning and review process.
You can add any other agents (1 or more) that you think are needed for the task, such as an implementer, a tester, a technical writer, etc.

## General flow

- The agent(s) that does the initial exploration phase needs to also look for relevant documentation, which includes specs, looking at `README.md` files, and possibly other `*.md` files.
- Implementing a change needs to also update the relevant documentation so that they are kept in sync and up to date.
- After all of the changes are done, an agent in the team should use the simplification skill to clean up implementation, documentation, etc.

## Feedback on previous use

1. Never mention the skill name of this skill or its origin in agent prompts - the agents don't need to know they were spawned from a "/team-work" skill. They just need their task.
2. Make prompts fully self-contained - every agent prompt should include all the context the agent needs to do its job. If the briefing is complete, there's no reason to go exploring for meta-context.

## Current task

$ARGUMENTS
