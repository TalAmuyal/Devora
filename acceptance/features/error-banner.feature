Feature: Global error banners
  Failed backend operations surface as dismissible error banners stacked above all UI, are written to the log file, and are recorded for test scraping.

  Scenario: A Rust-side error is surfaced as a global error banner
    Given a profile "Work" with 1 active workspaces
    And the task.json of workspace "ws-1" is malformed
    And the Workspace Hub is open
    Then an error banner containing "failed to parse" should be visible
    And the recorded errors should include "failed to parse"
