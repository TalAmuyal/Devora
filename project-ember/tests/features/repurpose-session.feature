@repurpose
Feature: Repurpose current session
  After finishing a task, the user often starts a follow-up task that needs the
  same kind of session. The "Repurpose Current Session" palette command swaps
  the workspace's task identity in place — new uid, started_at, and title —
  leaving the worktrees untouched.

  Background:
    Given Ember is running

  Scenario: Repurpose a clean, detached workspace
    Given a profile "Work" with 1 active workspaces
    And a session is opened for workspace "ws-1" from the Hub
    And the current task uid of workspace "ws-1" is noted
    When the user runs the "Repurpose Current Session" palette command
    Then the repurpose dialog should be visible
    And the repurpose dialog input should be pre-filled with "Task 1"
    When the user replaces the repurpose dialog input with "Follow-up task"
    And the user confirms the repurpose dialog
    Then the task.json title of workspace "ws-1" should be "Follow-up task"
    And the task uid of workspace "ws-1" should have changed
    And the active session title should be "Follow-up task"
    And the workspace repos should be clean and detached

  Scenario: A dirty worktree hard-blocks repurposing
    Given a profile "Work" with 1 active workspaces
    And workspace "ws-1" has uncommitted changes in repo "test-repo"
    And a session is opened for workspace "ws-1" from the Hub
    When the user runs the "Repurpose Current Session" palette command
    Then no repurpose dialog should appear
    And the recorded errors should include "test-repo: 1 untracked"
    And the task.json title of workspace "ws-1" should be "Task 1"

  Scenario: A worktree on a branch hard-blocks repurposing
    Given a profile "Work" with 1 active workspaces
    And repo "test-repo" of workspace "ws-1" is on branch "wip"
    And a session is opened for workspace "ws-1" from the Hub
    When the user runs the "Repurpose Current Session" palette command
    Then no repurpose dialog should appear
    And the recorded errors should include "test-repo: on branch 'wip'"
    And the task.json title of workspace "ws-1" should be "Task 1"

  Scenario: Cancelling the repurpose dialog changes nothing
    Given a profile "Work" with 1 active workspaces
    And a session is opened for workspace "ws-1" from the Hub
    And the current task uid of workspace "ws-1" is noted
    When the user runs the "Repurpose Current Session" palette command
    Then the repurpose dialog should be visible
    When the user cancels the repurpose dialog
    Then the task.json title of workspace "ws-1" should be "Task 1"
    And the task uid of workspace "ws-1" should be unchanged
    And no error banners should be visible

  Scenario: Repurposing a plain shell session shows an error
    Given a session exists
    When the user runs the "Repurpose Current Session" palette command
    Then no repurpose dialog should appear
    And the recorded errors should include "no associated workspace"
