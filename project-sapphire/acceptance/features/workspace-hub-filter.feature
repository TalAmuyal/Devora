Feature: The Workspace Hub's filter across the pages over it
  The hub stays alive beneath the Settings Hub, so its filter survives a round-trip.
  Ember's scenario isn't shared: its key steps leave the filter field before pressing a key, and Sapphire's keep the focus where it is, so here the user leaves the field first.

  Background:
    Given Devora is running with no sessions open

  Scenario: The hub's filter survives a round-trip through the Settings Hub
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user filters workspaces by "Task 2"
    Then the Workspace Hub should show 1 workspace items
    When the user leaves the hub's filter field
    And the user presses "P"
    Then the Settings Hub should be visible
    When the user presses "q"
    Then the Workspace Hub overlay should be present
    And the Workspace Hub should show 1 workspace items
