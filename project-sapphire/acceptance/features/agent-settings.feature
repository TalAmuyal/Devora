Feature: Agent settings at global and profile scopes
  The Agent card edits saved configuration and shows each effective value's source.

  Background:
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible

  Scenario: A typed percentage inherits and returns to its global value
    When the user opens the "User Defaults" settings detail
    Then the setting "agent.compact-threshold" should show the hint "80 · Devora default"
    When the user sets "agent.compact-threshold" to "65"
    Then the global config should store the Agent percentage 65
    When the user opens the "Work" settings detail
    Then the setting "agent.compact-threshold" should show the hint "65 · User Defaults"
    When the user sets "agent.compact-threshold" to "90"
    Then the profile "Work" config should store the Agent percentage 90
    When the user switches "agent.compact-threshold" to "Default"
    Then the setting "agent.compact-threshold" should show the hint "65 · User Defaults"

  Scenario: Multiline and empty values remain explicit until Default is selected
    When the user opens the "User Defaults" settings detail
    And the user saves Agent setting "agent.role" with text:
      """
      First paragraph

      Second paragraph
      """
    Then the global Agent setting "role" should contain exactly:
      """
      First paragraph

      Second paragraph
      """
    When the user opens the "Work" settings detail
    And the user sets "agent.role" to ""
    Then the profile "Work" config should have "agent.role" set to ""
    When the user switches "agent.role" to "Default"
    Then the profile "Work" config should not have "agent.role"
    When Sapphire is quit and relaunched
    And the user presses "P"
    And the user opens the "User Defaults" settings detail
    Then the global Agent setting "role" should contain exactly:
      """
      First paragraph

      Second paragraph
      """

  Scenario: Template paths resolve relative to the file defining them
    When the user opens the "User Defaults" settings detail
    And the user sets "agent.template-file" to "templates/global.md"
    Then the global config should have "agent.template-file" set to "templates/global.md"
    When the user opens the "Work" settings detail
    Then the Agent template path should resolve relative to the global config as "templates/global.md"
    When the user sets "agent.template-file" to "templates/profile.md"
    Then the Agent template path should resolve relative to profile "Work" as "templates/profile.md"

  Scenario: Literal input keys can be overridden, inherited and renamed
    When the user opens the "User Defaults" settings detail
    And the user adds Agent input "repo.notes" with value "global notes"
    Then the global Agent input "repo.notes" should equal "global notes"
    When the user opens the "Work" settings detail
    Then Agent input "repo.notes" should show "global notes" from "User Defaults"
    When the user saves Agent input "repo.notes" as "repo.notes" with value "profile notes"
    Then the profile "Work" Agent input "repo.notes" should equal "profile notes"
    When the user defaults Agent input "repo.notes"
    Then Agent input "repo.notes" should show "global notes" from "User Defaults"
    When the user opens the "User Defaults" settings detail
    And the user saves Agent input "repo.notes" as "repo.renamed" with value "renamed notes"
    Then the global Agent input "repo.renamed" should equal "renamed notes"
    And Agent input "repo.notes" should be absent

  Scenario: Invalid percentages and reserved input keys leave saved values intact
    When the user opens the "User Defaults" settings detail
    And the user sets "agent.compact-threshold" to "65"
    Then the global config should store the Agent percentage 65
    When the user sets "agent.compact-threshold" to "101"
    Then the recorded errors should include "whole percentage from 1 to 100"
    And the global config should store the Agent percentage 65
    When the user adds Agent input "agent.role" with value "wrong field"
    Then the recorded errors should include "named placeholders have their own Agent settings fields"
    And Agent input "agent.role" should be absent
