Feature: Global error banners
  Failed core calls and the errors the core reports surface as dismissible error banners stacked above all UI, are written to the log file, and are recorded for test scraping.
  The scenario shared with Ember is in `acceptance/features/error-banner.feature`; Ember's own reaches a failing call through Remove Task, which Sapphire doesn't have yet, so these fail the Workspace Hub's listing instead.

  Scenario: A failing core call shows a global error banner
    Given a profile "Work" with 1 active workspaces
    And the workspaces directory of the profile can't be read
    When the Workspace Hub is open
    Then an error banner containing "list_workspaces failed" should be visible
    And the log file should contain "[ERROR] list_workspaces failed"
    When the user dismisses the error banner
    Then no error banners should be visible
    And the recorded errors should include "list_workspaces failed"

  Scenario: The same error again counts on its banner
    Given a profile "Work" with 1 active workspaces
    And the workspaces directory of the profile can't be read
    When the Workspace Hub is open
    And the user presses "R"
    Then the error banner containing "list_workspaces failed" should count 2
    And the recorded errors should include "list_workspaces failed"

