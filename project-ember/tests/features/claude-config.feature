@claude-config
Feature: Claude launch configuration
  Devora-Ember resolves the Claude Code default model, model tiers, effort level, and permission mode from user/profile config (profile → user → Devora default, per key) and injects them into session shells as environment variables: the ANTHROPIC_* vars (default model + model tiers) are read by Claude Code natively, while the DEVORA_CCC_* vars (effort, permission mode) are turned by `ccc` into `--effort` / `--permission-mode` flags.
  A setting can be a value, None (omit the var so Claude Code uses its default), or unset (fall through).
  Values use a `:value:` sentinel so an omitted var shows as `::`.
  The settings are edited in the Settings Hub through the "Claude Launch Settings" card.

  Background:
    Given Ember is running

  Scenario: A configured model tier is exported to the session shell
    Given the global config sets the Claude "opus-model" to "claude-fable-5"
    When a new session is created
    And "echo OPUS=:$ANTHROPIC_DEFAULT_OPUS_MODEL:" is typed in the terminal
    Then the terminal should contain "OPUS=:claude-fable-5:"

  Scenario: The configured effort level is exported for ccc
    Given the global config sets the Claude "effort" to "max"
    When a new session is created
    And "echo EFFORT=:$DEVORA_CCC_EFFORT:" is typed in the terminal
    Then the terminal should contain "EFFORT=:max:"

  Scenario: A model tier set to None is left unset
    Given the global config sets the Claude "haiku-model" to None
    When a new session is created
    And "echo HAIKU=:$ANTHROPIC_DEFAULT_HAIKU_MODEL:" is typed in the terminal
    Then the terminal should contain "HAIKU=::"

  Scenario: Unset settings fall back to the Devora defaults
    When a new session is created
    And "echo DEFAULTS=:$ANTHROPIC_DEFAULT_OPUS_MODEL:$DEVORA_CCC_EFFORT:" is typed in the terminal
    Then the terminal should contain "DEFAULTS=:claude-opus-4-8:xhigh:"

  Scenario: The default model is exported as opusplan by default
    When a new session is created
    And "echo MODEL=:$ANTHROPIC_MODEL:" is typed in the terminal
    Then the terminal should contain "MODEL=:opusplan:"

  Scenario: The permission mode is exported for ccc, defaulting to plan
    When a new session is created
    And "echo PM=:$DEVORA_CCC_PERMISSION_MODE:" is typed in the terminal
    Then the terminal should contain "PM=:plan:"

  Scenario: A configured permission mode is exported for ccc
    Given the global config sets the Claude "permission-mode" to "acceptEdits"
    When a new session is created
    And "echo PM=:$DEVORA_CCC_PERMISSION_MODE:" is typed in the terminal
    Then the terminal should contain "PM=:acceptEdits:"

  Scenario: A permission mode set to None is left unset
    Given the global config sets the Claude "permission-mode" to None
    When a new session is created
    And "echo PM=:$DEVORA_CCC_PERMISSION_MODE:" is typed in the terminal
    Then the terminal should contain "PM=::"

  Scenario: Setting the effort level from the Settings Hub
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    And the user sets the effort level to "low"
    Then the global config should have the Claude "effort" set to "low"

  Scenario: Turning off the default model from the Settings Hub
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    And the user turns the default model off
    Then the global config should have the Claude "default-model" set to None

  Scenario: Setting the permission mode from the Settings Hub
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    And the user sets the permission mode to "acceptEdits"
    Then the global config should have the Claude "permission-mode" set to "acceptEdits"
