@settings-claude
Feature: Claude settings and the shared Asana token
  Claude values are stored at the selected scope, while the Asana credential is shared by both scopes.
  The acceptance harness injects an in-memory credential store instead of the login keychain.

  Background:
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible

  Scenario: Explicit None stops inheritance and Default restores it
    When the user opens the "User Defaults" settings detail
    And the user sets Claude "sonnet-model" to custom "future-model-id"
    Then the global config should have the Claude "sonnet-model" set to "future-model-id"
    When the user opens the "Work" settings detail
    Then Claude "sonnet-model" should show "Default" with hint "→ future-model-id"
    When the user switches Claude "sonnet-model" to "None"
    Then the profile "Work" Claude "sonnet-model" should be None on disk
    When the user switches Claude "sonnet-model" to "Default"
    Then the profile "Work" config should not have "claude.sonnet-model"
    And Claude "sonnet-model" should show "Default" with hint "→ future-model-id"

  Scenario: A model suggestion commits its value
    When the user opens the "User Defaults" settings detail
    And the user switches Claude "opus-model" to "Custom"
    And the user selects the Claude "opus-model" suggestion "claude-sonnet-5"
    Then the global config should have the Claude "opus-model" set to "claude-sonnet-5"

  Scenario: An uncommitted custom draft survives an unrelated write
    When the user opens the "User Defaults" settings detail
    And the user switches Claude "opus-model" to "Custom"
    And the user types Claude "opus-model" draft "not-saved-yet"
    And the user sets the effort level to "low"
    Then the global config should have the Claude "effort" set to "low"
    And Claude "opus-model" should have draft "not-saved-yet"
    And the user config should not store Claude "opus-model"

  Scenario: A blank custom commit returns a profile to inheritance
    When the user opens the "Work" settings detail
    And the user sets Claude "permission-mode" to custom "future-mode"
    Then the profile "Work" config should have "claude.permission-mode" set to "future-mode"
    When the user sets Claude "permission-mode" to custom "   "
    Then the profile "Work" config should not have "claude.permission-mode"
    And Claude "permission-mode" should show "Default" with hint "→ plan"

  Scenario: Both scopes share the token without storing it in config
    When the user opens the "User Defaults" settings detail
    And the user switches "task-tracker.provider" to "Asana"
    Then the Asana token should be "Not set"
    When the user saves the fixture Asana token
    Then the Asana token should be "Set"
    And the Asana token draft should be empty
    When the user opens the "Work" settings detail
    Then the Asana token should be "Set"
    When the user clears the Asana token
    Then the Asana token should be "Not set"
    When the user opens the "User Defaults" settings detail
    Then the Asana token should be "Not set"
    And neither scope should store the fixture token on disk
    When the user switches "task-tracker.provider" to "None"
    Then the Asana token row should be hidden
