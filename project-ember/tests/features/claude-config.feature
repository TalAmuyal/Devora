@claude-config
Feature: Claude model and effort configuration
  Devora-Ember resolves the Claude Code model tiers and effort level from user/profile config (profile → user → Devora default, per key) and injects them into session shells as environment variables: the model tiers via the ANTHROPIC_DEFAULT_*_MODEL vars that Claude Code reads natively, and the effort via DEVORA_CCC_EFFORT which `ccc` turns into `--effort`.
  A setting can be a value, None (omit the var so Claude Code uses its default), or unset (fall through).
  Values use a `:value:` sentinel so an omitted var shows as `::`.

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
