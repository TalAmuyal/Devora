@real-claude
Feature: Claude Code hook integration
  Claude Code runs inside an Ember terminal session. When it performs
  tool use, configured hooks fire and can interact with the Ember UI
  through the IPC HTTP server.

  Background:
    Given Ember is running
    And a workspace session exists with a Claude Code hook

  Scenario: Claude Code hook triggers a crit overlay
    When Claude Code runs and triggers a tool use
    Then a panel overlay should appear on the active session

  Scenario: Claude Code hook receives session context
    When Claude Code runs and triggers a tool use
    Then the hook script should have received DEVORA_PTY_ID
    And the hook script should have received DEVORA_IPC_PORT
