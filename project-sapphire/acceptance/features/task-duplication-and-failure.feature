Feature: Duplicating a task, and a creation that fails
  Ember has no scenarios for these: Duplicate opens the New Task dialog prefilled from an active workspace's task, and a failed creation stays on its tab, with its error, until it is closed.

  Background:
    Given Devora is running with no sessions open

  Scenario: Duplicate opens the New Task dialog prefilled from the task
    Given an origin-backed profile "Work" with repo "test-repo"
    And the Workspace Hub is open
    And the user creates a task "Original" selecting repo "test-repo"
    And the task creation should complete
    And the Workspace Hub is open
    When the user duplicates the focused workspace
    Then the New Task form should be titled "Duplicate Task" with "Original" and repo "test-repo" selected
    When the user creates the task in the New Task form
    Then the task creation should complete
    And the active session should be connected
    And workspace "ws-2" should be active with title "Original"

  Scenario: A failed creation shows its error and banner, and closing it tears its tab down
    Given an origin-backed profile "Work" with repo "test-repo"
    And the origin of repo "test-repo" is gone
    And the Workspace Hub is open
    When the user creates a task "Doomed" selecting repo "test-repo"
    Then the task creation should fail with "git fetch failed for test-repo"
    And an error banner containing "git fetch failed for test-repo" should be visible
    And the recorded errors should include "git fetch failed for test-repo"
    When the user presses "q"
    Then the task-creation tab should close
    And no workspace "ws-1" should exist
