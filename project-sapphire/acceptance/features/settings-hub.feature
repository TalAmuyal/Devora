Feature: Settings Hub settings cards
  The Settings Hub's cards edit the config settings at the User Defaults' scope (the global config) or a profile's (its `config.json`).
  "Default" removes the key, so that the scope inherits it, and the inherited value shows as a hint.

  Background:
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible

  Scenario: A profile's value overrides the User Defaults' and Default inherits it again
    When the user opens the "User Defaults" settings detail
    And the user sets "terminal.default-app" to "nvim"
    Then the global config should have "terminal.default-app" set to "nvim"
    When the user opens the "Work" settings detail
    Then the setting "terminal.default-app" should show the hint "→ nvim"
    When the user sets "terminal.default-app" to "hx"
    Then the profile "Work" config should have "terminal.default-app" set to "hx"
    And the global config should have "terminal.default-app" set to "nvim"
    When the user switches "terminal.default-app" to "Default"
    Then the profile "Work" config should not have "terminal.default-app"
    And the setting "terminal.default-app" should show the hint "→ nvim"

  Scenario: An On/Off setting falls back to Devora's default
    When the user opens the "Work" settings detail
    Then the setting "terminal.git-shortcuts" should show the hint "→ on"
    When the user switches "terminal.git-shortcuts" to "Off"
    Then the profile "Work" config should have "terminal.git-shortcuts" set to false
    When the user switches "terminal.git-shortcuts" to "Default"
    Then the profile "Work" config should not have "terminal.git-shortcuts"
    And the setting "terminal.git-shortcuts" should show the hint "→ on"

  Scenario: A title prefix keeps its trailing space
    When the user opens the "User Defaults" settings detail
    And the user sets "pr.title-prefix" to "[ENG] "
    Then the global config should have "pr.title-prefix" set to "[ENG] "

  Scenario: Committing an empty value reverts to Default
    When the user opens the "Work" settings detail
    And the user sets "prepare-command" to "mise install"
    Then the profile "Work" config should have "prepare-command" set to "mise install"
    When the user sets "prepare-command" to ""
    Then the profile "Work" config should not have "prepare-command"
    And the setting "prepare-command" should show the hint "Uses Devora default"

  Scenario: The Asana settings show while the provider resolves to Asana
    When the user opens the "User Defaults" settings detail
    Then the setting "task-tracker.asana.project-id" should not be shown
    When the user switches "task-tracker.provider" to "Asana"
    Then the global config should have "task-tracker.provider" set to "asana"
    When the user opens the "Work" settings detail
    Then the setting "task-tracker.asana.project-id" should be shown
    When the user switches "task-tracker.provider" to "None"
    Then the profile "Work" config should have "task-tracker.provider" set to ""
    And the setting "task-tracker.asana.project-id" should not be shown
