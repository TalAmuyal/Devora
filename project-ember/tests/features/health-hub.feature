@health-hub
Feature: Health Hub
  The Health Hub renders the bundled `debi health --json`, showing dependencies, credentials, config, and zsh completion.
  It opens from the Command Palette and from the Workspace Hub.

  Background:
    Given Ember is running

  Scenario: Open the Health Hub from the Command Palette
    Given a profile "Work" with 1 active workspaces
    And the Command Palette is open
    When the user filters commands by "health"
    Then the Command Palette should show 1 command
    When the user presses "Enter"
    And the Health Hub finishes loading
    Then the Health Hub should be visible
    And the Health Hub should show the "Required dependencies" section
    And the Health Hub should show the "Credentials" section
    And the Health Hub should show the "Configuration" section
    And the Health Hub should report the config file as found
    And the Health Hub should not list kitty
    And the Health Hub should show a version

  Scenario: Open the Health Hub from the Workspace Hub with H
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "H"
    And the Health Hub finishes loading
    Then the Health Hub should be visible
    And the Health Hub should show the "Required dependencies" section
