Feature: Global error banners
  Failed core calls and the errors the core reports surface as dismissible error banners stacked above all UI, are written to the log file, and are recorded for test scraping.
  Ember's scenarios (`project-ember/tests/features/error-banner.feature`) go through the Workspace Hub, which Sapphire doesn't have yet; these reach the same paths through the sidebar's listing.

  Scenario: A failing core call shows a global error banner
    Given a profile "Work" with 1 active workspaces
    And the workspaces directory of the profile can't be read
    When the workspaces are reloaded
    Then an error banner containing "list_workspaces failed" should be visible
    And the log file should contain "[ERROR] list_workspaces failed"
    When the user dismisses the error banner
    Then no error banners should be visible
    And the recorded errors should include "list_workspaces failed"

  Scenario: The same error again counts on its banner
    Given a profile "Work" with 1 active workspaces
    And the workspaces directory of the profile can't be read
    When the workspaces are reloaded
    And the workspaces are reloaded
    Then the error banner containing "list_workspaces failed" should count 2
    And the recorded errors should include "list_workspaces failed"

  Scenario: A Rust-side error is surfaced as a global error banner
    Given a profile "Work" with 1 active workspaces
    And the task.json of workspace "ws-1" is malformed
    When the workspaces are reloaded
    Then an error banner containing "failed to parse" should be visible
    And the log file should contain "[ERROR] failed to parse"
    And the recorded errors should include "failed to parse"
