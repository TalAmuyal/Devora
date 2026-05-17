# Devora Ember — Deferred Items

Items that are out of scope for the initial PoC but planned for future implementation.

## Split Panes (Multi-Panel)

- Split panes within session tabs (Cmd+Shift+\, Cmd+Shift+-, directional navigation with Cmd+Shift+H/J/K/L)
- The session tab architecture supports this: each session would hold a split layout with multiple panes instead of a single terminal pane
- Pane types: terminal, rendered markdown, HTML page
- When a pane's process exits, that pane should close. If it was the last pane in the session tab, the tab should also close. Currently only single-pane sessions exist, so process exit closes the entire tab.

## Overlay Modes

- Pop-up overlay (~80% window, centered) — for adding worktrees to existing workspaces
- Dialog overlay (small centered) — for yes/no prompts and confirmations

## Workspace Management

- Workspace deletion from the Workspace Hub
- Workspace deactivation from the Workspace Hub

## Tab Bar

- Tab drag-and-drop reordering

## Crit Integration

- If the lightweight socket/HTTP approach for the crit wrapper proves insufficient, implement a more robust IPC mechanism for embedded Crit panel overlays
