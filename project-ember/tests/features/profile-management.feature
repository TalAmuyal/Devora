Feature: Profile management
  Profiles can be created, registered, switched, and deleted in Devora-Ember:
  via the first-run welcome card, the Settings Hub overlay (P from the hub,
  the hub's profile dropdown, or the hub's burger menu), and the command palette.
  The first-run welcome and the dropdown's "New Profile…" are in `acceptance/features/first-run-and-new-profile.feature`, and the Settings Hub's profile scenarios in `acceptance/features/settings-hub-profiles.feature`, both shared with Sapphire.

  Scenario: Hub profile dropdown items are clickable at their on-screen position
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user clicks the profile dropdown
    Then the profile dropdown should list profile "Work"
    And the profile dropdown items should be clickable at their on-screen position

  Scenario: Command palette offers profile commands
    Given a profile "Work" with 1 active workspace with worktrees
    And a profile "Personal" with 1 active workspace with worktrees
    And the Command Palette is open
    When the user filters commands by "profile"
    Then the palette should include the command "New Profile"
    And the palette should include the command "Settings / Manage Profiles"
    And the palette should include the command "Switch Profile: Personal"
    When the user filters commands by "Switch Profile: Personal"
    And the user presses "Enter"
    Then the Workspace Hub overlay should be present
    And the Workspace Hub should show 1 workspace items

  # The hub stays alive beneath Settings, so its filter is preserved across the round-trip.
  # Ember's key steps leave the filter field before pressing, which Sapphire's don't, so this one isn't shared.
  Scenario: Hub filter text survives a round-trip through Settings
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user filters workspaces by "Task 2"
    Then the Workspace Hub should show 1 workspace items
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user presses "q"
    Then the Workspace Hub overlay should be present
    And the Workspace Hub should show 1 workspace items
