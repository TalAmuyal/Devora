Feature: Workspace Hub
  The Workspace Hub shows workspaces for the active profile.
  Users navigate with keyboard shortcuts, filter, and open workspaces.

  Background:
    Given Ember is running

  Scenario: Filter the New Task form repo list
    Given a profile "Work" with repos "repo-alpha, repo-beta, repo-gamma"
    And the Workspace Hub is open
    When the user opens the New Task form
    Then the New Task form should show 3 repos
    When the user filters the New Task form repo list by "beta"
    Then the New Task form should show 1 repo

  Scenario: Switching profiles loads workspace status
    Given a profile "Work" with 2 active workspaces with worktrees
    And a profile "Personal" with 1 active workspace with worktrees
    And the Workspace Hub is open
    Then the Workspace Hub should show 2 workspace items
    And the detail panel should show repo status
    When the user switches to profile "Personal"
    Then the Workspace Hub should show 1 workspace items
    And the detail panel should show repo status

  Scenario: Header profile dropdown and burger menu are vertically aligned
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    Then the profile dropdown and burger menu should be vertically aligned

  Scenario: Save loading latencies writes a diagnostics file
    Given a profile "Work" with 2 active workspaces
    And the Workspace Hub is open
    When the user clicks the save loading latencies button
    Then a profiling report file should exist in the diagnostics directory

  Scenario: Typing a shortcut key in the New Task form is not intercepted
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    And the New Task form is open
    When the user types "n" into the focused task title input
    Then the New Task form should still be visible
    And the keypress should not have been intercepted

  # The dropdown is a popup layer, so Escape dismisses only it; the hub underneath stays open and a second Escape closes the hub.
  Scenario: Escape with the profile dropdown open closes the dropdown but keeps the hub
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user clicks the profile dropdown
    And the user presses "Escape"
    Then the profile dropdown should not be visible
    And the Workspace Hub overlay should be present
    When the user presses "Escape"
    Then the Workspace Hub should not be visible

  # A hub hotkey pressed with the dropdown open removes the popup layer cleanly, so the next q closes the hub (not a phantom dropdown).
  Scenario: A hub hotkey with the profile dropdown open closes the dropdown cleanly
    Given a profile "Work" with 2 active and 1 inactive workspaces
    And the Workspace Hub is open
    When the user clicks the profile dropdown
    And the user presses "3"
    Then the profile dropdown should not be visible
    And the active category should be "All"
    When the user presses "q"
    Then the Workspace Hub should not be visible
