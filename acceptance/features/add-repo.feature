@add-repo
Feature: Add a repo to a workspace
  The user can add another repo (git worktree) to the workspace of the active session — the
  Devora equivalent of `debi add`. Ember opens Add Repo from the Command Palette; Sapphire, until
  it has a palette, from the Workspace Hub's detail panel on that workspace. Progress streams in
  the dialog, the new worktree appears under the workspace, and a cancel removes it again.

  Background:
    Given Devora is running with no sessions open

  Scenario: Add another worktree of a repo to the active workspace
    Given an origin-backed profile "Work" with repo "test-repo"
    And the Workspace Hub is open
    When the user creates a task "Main work" selecting repo "test-repo"
    Then the task creation should complete
    And the active session should be connected
    When the user opens Add Repo for the active session's workspace
    And the user adds repo "test-repo" with postfix "ref"
    Then the worktree "test-repo-ref" should exist in workspace "ws-1"

  Scenario: Filter the repo list in the Add Repo dialog
    Given an origin-backed profile "Work" with repo "test-repo"
    And an additional origin-backed repo "other-repo"
    And the Workspace Hub is open
    When the user creates a task "Main work" selecting repo "test-repo"
    Then the task creation should complete
    And the active session should be connected
    When the user opens Add Repo for the active session's workspace
    And the user filters the add-repo list by "other"
    Then the add-repo list should show 1 repo
    When the user submits the add-repo dialog with postfix "ref"
    Then the worktree "other-repo-ref" should exist in workspace "ws-1"

  Scenario: Cancelling an add while it prepares removes the new worktree
    Given an origin-backed profile "Work" with repo "test-repo"
    And the Workspace Hub is open
    When the user creates a task "Main work" selecting repo "test-repo"
    Then the task creation should complete
    And the active session should be connected
    Given the prepare-command sleeps
    When the user opens Add Repo for the active session's workspace
    And the user starts adding repo "test-repo" with postfix "ref"
    And the user cancels adding the repo once it is preparing
    Then the Add Repo dialog should close
    And no worktree "test-repo-ref" should exist in workspace "ws-1"
