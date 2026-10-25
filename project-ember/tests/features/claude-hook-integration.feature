@real-claude
Feature: Claude Code hook integration
  Claude Code runs inside an Ember terminal session. When it performs
  tool use, configured hooks fire and can interact with the Ember UI
  through the IPC HTTP server.

  Background:
    Given Ember is running

  Scenario: Claude Code hook opens a preview pane
    Given a workspace session exists with a Claude Code hook
    When Claude Code runs and triggers a tool use
    Then the hook should open a preview pane on the active session

  Scenario: Claude Code hook receives session context
    Given a workspace session exists with a Claude Code hook
    When Claude Code runs and triggers a tool use
    Then the hook script should have received DEVORA_PTY_ID
    And the hook script should have received DEVORA_IPC_PORT

  Scenario: Claude Code hook preview opens only on its owning tab
    Given 2 sessions exist
    And the first session has a workspace with a PostToolUse Claude Code hook
    And the user switches to the second session
    When Claude Code triggers a tool use in the first session
    Then the hook should open a preview pane on the first session
    And the second session should have 0 preview panes
