# Rich Text Editor

## Problem statement

Using the native textarea element for editing text files is a poor user experience.
It lacks features like syntax highlighting, line numbers, proper handling of indentation, and a huge number or other expected features.
This makes it difficult to edit *.md files, which is very common in Devora, since a lot of the user interaction with Devora happens through markdown files (e.g., `CLAUDE.md`, `TASK.md`, etc.).

## Vision

There should be a `RichTextEditor` component that is powered by a headless Neovim instance (or something else to that effect).
Neovim provides a powerful editing experience with tons of features and plugins, and can be embedded in a web app.
