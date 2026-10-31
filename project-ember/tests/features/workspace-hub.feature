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

  Scenario: Header profile dropdown and burger menu are vertically aligned
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    Then the profile dropdown and burger menu should be vertically aligned

  Scenario: Typing a shortcut key in the New Task form is not intercepted
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    And the New Task form is open
    When the user types "n" into the focused task title input
    Then the New Task form should still be visible
    And the keypress should not have been intercepted
