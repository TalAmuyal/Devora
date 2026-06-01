Feature: Workspace Hub
  The Workspace Hub shows workspaces for the active profile.
  Users navigate with keyboard shortcuts, filter, and open workspaces.

  Background:
    Given Ember is running

  Scenario: Hub shows workspaces from profile
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    Then the Workspace Hub should show 3 workspace items

  Scenario: Navigate workspaces with j/k
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user presses "j" 2 times
    Then the focused workspace should be "ws-3"
    When the user presses "k"
    Then the focused workspace should be "ws-2"

  Scenario: Open workspace with Enter
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user presses "j"
    And the user presses "Enter"
    Then the Workspace Hub should not be visible
    And there should be 1 session

  Scenario: Close hub with q
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "q"
    Then the Workspace Hub should not be visible

  Scenario: Filter workspaces
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user filters workspaces by "Task 2"
    Then the Workspace Hub should show 1 workspace items

  Scenario: Switch category tabs
    Given a profile "Work" with 2 active and 1 inactive workspaces
    And the Workspace Hub is open
    Then the Workspace Hub should show 2 workspace items
    When the user presses "3"
    Then the Workspace Hub should show 3 workspace items
    And the active category should be "All"
    When the user presses "2"
    Then the Workspace Hub should show 1 workspace items
    And the active category should be "Inactive"

  Scenario: Toggle cheatsheet with ?
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "?"
    Then the cheatsheet should be visible
    When the user presses "?"
    Then the cheatsheet should not be visible

  Scenario: Switching profiles loads workspace status
    Given a profile "Work" with 2 active workspaces with worktrees
    And a profile "Personal" with 1 active workspace with worktrees
    And the Workspace Hub is open
    Then the Workspace Hub should show 2 workspace items
    And the detail panel should show repo status
    When the user switches to profile "Personal"
    Then the Workspace Hub should show 1 workspace items
    And the detail panel should show repo status

  Scenario: Hub shell renders immediately on load
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is loading
    Then the hub header should be visible
    And the hub legend should be visible
    And the hub search bar should be visible

  Scenario: Save loading latencies writes a diagnostics file
    Given a profile "Work" with 2 active workspaces
    And the Workspace Hub is open
    When the user clicks the save loading latencies button
    Then a profiling report file should exist in the diagnostics directory

  Scenario: Hub is navigable immediately after opening
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user presses "j"
    Then the focused workspace should be "ws-2"
    When the user presses "k"
    Then the focused workspace should be "ws-1"
