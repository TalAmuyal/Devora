Feature: Workspace Hub
  The Workspace Hub shows workspaces for the active profile.
  Users navigate with keyboard shortcuts, filter, and open workspaces.

  Background:
    Given Ember is running

  Scenario: Header profile dropdown and burger menu are vertically aligned
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    Then the profile dropdown and burger menu should be vertically aligned
