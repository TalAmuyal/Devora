@add-repo
Feature: Add a repo to the current workspace
  From the Command Palette the user can add another repo (git worktree) to the workspace of the
  active session — the Ember equivalent of `debi add`. Progress streams in a dialog and the new
  worktree appears under the workspace.

  Background:
    Given Ember is running

  Scenario: Add another worktree of a repo to the active workspace
    Given an origin-backed profile "Work" with repo "test-repo"
    And the Workspace Hub is open
    When the user creates a task "Main work" selecting repo "test-repo"
    Then the task creation should complete
    And the active session should be connected
    When the user runs the "Add Repo to Workspace" palette command
    And the user adds repo "test-repo" with postfix "ref"
    Then the worktree "test-repo-ref" should exist in workspace "ws-1"
