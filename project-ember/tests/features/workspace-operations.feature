Feature: Workspace Hub - Remove Task and Delete operations
  Active workspaces can have their task removed (making them inactive).
  Inactive and invalid workspaces can be permanently deleted.

  Background:
    Given Ember is running

  # --- Remove Task (active workspaces) ---

  Scenario: Remove task from clean active workspace without confirmation
    Given a profile "Work" with 1 active workspace with worktrees
    And the workspace repos are clean and detached
    And the Workspace Hub is open
    When the user clicks the "Remove Task" button
    Then no confirmation dialog should appear
    And the task.json should no longer exist
    And the workspace directory should still exist
    And the worktree directory should still exist
    And the worktree should still be registered in the source repo
    And the Workspace Hub should show 0 workspace items
    When the user presses "2"
    Then the Workspace Hub should show 1 workspace items

  Scenario: Remove task from dirty active workspace with confirmation
    Given a profile "Work" with 1 active workspace with worktrees
    And workspace "ws-1" has uncommitted changes in repo "test-repo"
    And the Workspace Hub is open
    When the user clicks the "Remove Task" button
    Then a confirmation dialog should be visible
    When the user confirms the dialog
    Then the task.json should no longer exist
    And the workspace repos should be clean and detached
    And the workspace directory should still exist
    And the worktree directory should still exist
    And the worktree should still be registered in the source repo

  Scenario: Cancel removing task from dirty active workspace
    Given a profile "Work" with 1 active workspace with worktrees
    And workspace "ws-1" has uncommitted changes in repo "test-repo"
    And the Workspace Hub is open
    When the user clicks the "Remove Task" button
    Then a confirmation dialog should be visible
    When the user cancels the dialog
    Then the task.json should still exist
    And the workspace should still have uncommitted changes
    And the worktree directory should still exist

  # --- Delete (inactive/invalid workspaces) ---

  Scenario: Delete inactive workspace without confirmation
    Given a profile "Work" with 1 inactive workspace with worktrees
    And the Workspace Hub is open
    When the user presses "2"
    And the user clicks the "Delete" button
    Then no confirmation dialog should appear
    And the workspace directory should not exist
    And the worktree should not be registered in the source repo
    And the source repo should still exist
    And the workspace list should show 0 items

  Scenario: Delete invalid workspace without confirmation
    Given a profile "Work" with 1 invalid workspace
    And the Workspace Hub is open
    When the user presses "2"
    And the user clicks the "Delete" button
    Then no confirmation dialog should appear
    And the workspace directory should not exist
    And the workspace list should show 0 items
