@task-creation
Feature: Non-blocking task creation with reuse and refresh
  Creating a task never freezes the window: the Workspace Hub is replaced by a new tab whose
  panel overlay streams creation progress until the terminal connects. When an inactive workspace
  matches the selected repos it is reused and refreshed (its dependency cache is preserved),
  otherwise a fresh workspace is built. Creation can be cancelled mid-flight.

  Background:
    Given Ember is running

  Scenario: A task with no reusable workspace opens a progress tab and builds fresh
    Given an origin-backed profile "Work" with repo "test-repo"
    And the prepare-command writes a marker
    And the Workspace Hub is open
    When the user creates a task "Fix login" selecting repo "test-repo"
    Then the Workspace Hub should not be visible
    And a task-creation progress overlay should be visible
    And the task creation should complete
    And the active session should be connected
    And workspace "ws-1" should be active with title "Fix login"
    And the prepare marker should exist in workspace "ws-1" repo "test-repo"

  Scenario: A task reuses a matching inactive workspace and refreshes it
    Given an origin-backed profile "Work" with repo "test-repo"
    And an inactive workspace "ws-1" with a worktree of "test-repo"
    And a cached file "node_modules_marker" in workspace "ws-1" repo "test-repo"
    And the origin of repo "test-repo" has advanced
    And the prepare-command writes a marker
    And the Workspace Hub is open
    When the user creates a task "Follow up" selecting repo "test-repo"
    Then the task creation should complete
    And the active session should be connected
    And workspace "ws-1" should be active with title "Follow up"
    And no workspace "ws-2" should exist
    And the worktree of "test-repo" in workspace "ws-1" should be at the latest origin commit
    And the cached file "node_modules_marker" should exist in workspace "ws-1" repo "test-repo"
    And the prepare marker should exist in workspace "ws-1" repo "test-repo"

  Scenario: Cancelling a fresh creation tears down the tab and removes the workspace
    Given an origin-backed profile "Work" with repo "test-repo"
    And the prepare-command sleeps
    And the Workspace Hub is open
    When the user creates a task "Will cancel" selecting repo "test-repo"
    And the user cancels the task creation once it is preparing
    Then the task-creation tab should close
    And no workspace "ws-1" should exist

  Scenario: Cancelling a reused creation reverts it to inactive, keeping the cache
    Given an origin-backed profile "Work" with repo "test-repo"
    And an inactive workspace "ws-1" with a worktree of "test-repo"
    And a cached file "node_modules_marker" in workspace "ws-1" repo "test-repo"
    And the prepare-command sleeps
    And the Workspace Hub is open
    When the user creates a task "Will cancel" selecting repo "test-repo"
    And the user cancels the task creation once it is preparing
    Then the task-creation tab should close
    And workspace "ws-1" should be inactive
    And the cached file "node_modules_marker" should exist in workspace "ws-1" repo "test-repo"
