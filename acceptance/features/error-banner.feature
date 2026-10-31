Feature: Global error banners
  Failed backend operations surface as dismissible error banners stacked above all UI, are written to the log file, and are recorded for test scraping.

  Scenario: A failing backend command shows a global error banner
    Given a profile "Work" with 1 active workspace with worktrees
    And the workspace repos are clean and detached
    And the Workspace Hub is open
    And the detail panel should show repo status
    When the worktree repo of workspace "ws-1" becomes corrupted
    And the user clicks the "Remove Task" button
    Then an error banner containing "remove_task failed" should be visible
    When the user dismisses the error banner
    Then no error banners should be visible
    And the recorded errors should include "remove_task failed"

  Scenario: A Rust-side error is surfaced as a global error banner
    Given a profile "Work" with 1 active workspaces
    And the task.json of workspace "ws-1" is malformed
    And the Workspace Hub is open
    Then an error banner containing "failed to parse" should be visible
    And the recorded errors should include "failed to parse"
