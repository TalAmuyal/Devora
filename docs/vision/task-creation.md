# Task Creation Vision

## Problem statement

When working on a new task, users open a Devora Session, edit the workspace's root CLAUDE.md and then prompt Claude (using `ccc`) to do the work.

Prompting Claude is done in one of two ways:
1. Create a prompt file (TASK.md, NEXT.md, etc.) in the root of the workspace and run `ccc < prompt-file` to have Claude read the file and respond in the terminal
2. Run `ccc` with no input file, and then author the prompt directly in Claude Code's interactive prompt editor

The prompt-file approach gives a better editing experience when done using Neovim, and yields a more persistent record of the prompt.
The interactive approach has a better Claude Code integration and can auto-complete skills/slash-commands and "@" mentions.

In addition, there is no easy way to combine snippets, skills, etc. into a single prompt.

## Vision

The "New Task" card of the Workspace Hub should use a `PromptEditor` component that extends `RichTextEditor` (see @rich-text-editor.md) and provides the following features:
- Completions on `@` for file/dir paths and `/` for skill names
	- Maybe through a Neovim plugin or a component-level implementation
- A new prompt templating system which will allow users to easily combine snippets, skills, and other building blocks into a single prompt
	- This could be a simple syntax like `{{snippet:snippet-name}}` or `{{skill:skill-name}}` that gets replaced with the actual content right before the prompt is fed to Claude Code

The `PromptEditor` should be a reusable component that can be used for other use-cases, like writing commit messages.
