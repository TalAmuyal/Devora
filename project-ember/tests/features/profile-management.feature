Feature: Profile management
  Profiles can be created, registered, switched, and deleted in Devora-Ember:
  via the first-run welcome card, the Profile Manager overlay (P from the hub),
  the hub's profile dropdown, and the command palette.

  Scenario: First-run welcome appears and cannot be dismissed
    Given no profiles are configured
    And the Workspace Hub is open
    Then the first-run welcome card should be visible
    And pressing "q" should not close the Workspace Hub

  Scenario: Creating the first profile from the welcome card
    Given no profiles are configured
    And the Workspace Hub is open
    When the user enters profile name "Work" and path "work" under the fixture root
    And the user submits the profile form
    Then the profile dropdown should be labeled "Work"
    And the directory "work/repos" should exist under the fixture root
    And the directory "work/workspaces" should exist under the fixture root
    And the global config should list 1 profile

  Scenario: Opening the Profile Manager with P and returning with q
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Profile Manager should be visible
    And the Profile Manager should list 1 profile and a New Profile row
    When the user presses "q"
    Then the Workspace Hub overlay should be present

  Scenario: Creating a new profile from the Profile Manager
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Profile Manager should list 1 profile and a New Profile row
    When the user presses "n"
    And the user enters profile name "Personal" and path "personal" under the fixture root
    Then the profile form should report a new profile
    When the user submits the profile form
    Then the Workspace Hub overlay should be present
    And the profile dropdown should be labeled "Personal"
    And the global config should list 2 profiles

  Scenario: Registering an existing profile directory
    Given a profile "Work" with 1 active workspaces
    And an unregistered initialized profile directory "Legacy"
    And the Workspace Hub is open
    When the user presses "P"
    Then the Profile Manager should list 1 profile and a New Profile row
    When the user presses "n"
    And the user enters path "Legacy" under the fixture root in the profile form
    Then the profile form should detect the existing profile "Legacy"
    And the profile form submit button should read "Register Profile"
    When the user submits the profile form
    Then the profile dropdown should be labeled "Legacy"
    And the global config should list 2 profiles

  Scenario: Switching the active profile from the Profile Manager
    Given a profile "Work" with 2 active workspaces with worktrees
    And a profile "Personal" with 1 active workspace with worktrees
    And the Workspace Hub is open
    Then the Workspace Hub should show 2 workspace items
    When the user presses "P"
    And the user presses "j"
    Then the focused profile should be "Personal"
    When the user presses "Enter"
    Then the Workspace Hub overlay should be present
    And the Workspace Hub should show 1 workspace items
    And the profile dropdown should be labeled "Personal"

  Scenario: Deleting a profile removes it from the registry but not from disk
    Given a profile "Work" with 1 active workspace with worktrees
    And a profile "Personal" with 1 active workspace with worktrees
    And the Workspace Hub is open
    When the user presses "P"
    And the user presses "j"
    And the user presses "d"
    Then a confirmation dialog should be visible
    And the confirmation dialog should mention "remains on disk"
    When the user confirms the dialog
    Then the Profile Manager should list 1 profile and a New Profile row
    And the profile directory "Personal" should still exist on disk
    And the global config should list 1 profile

  Scenario: Deleting a profile is blocked while it has open sessions
    Given a profile "Work" with 1 active workspace with worktrees
    And the Workspace Hub is open
    When the user presses "Enter"
    Then a workspace session should be open
    When the Workspace Hub is open
    And the user presses "P"
    And the user presses "d"
    Then a notice dialog titled "Cannot delete profile" should be visible
    And the notice dialog should have no cancel button
    When the user confirms the dialog
    Then the Profile Manager should list 1 profile and a New Profile row
    And the global config should list 1 profile

  Scenario: Hub profile dropdown offers New Profile and Manage Profiles
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user clicks the profile dropdown
    Then the profile dropdown should list profile "Work"
    And the profile dropdown should offer "New Profile…" and "Manage Profiles…"
    When the user clicks the "Manage Profiles…" dropdown action
    Then the Profile Manager should be visible

  Scenario: Command palette offers profile commands
    Given a profile "Work" with 1 active workspace with worktrees
    And a profile "Personal" with 1 active workspace with worktrees
    And the Command Palette is open
    When the user filters commands by "profile"
    Then the palette should include the command "New Profile"
    And the palette should include the command "Manage Profiles"
    And the palette should include the command "Switch Profile: Personal"
    When the user filters commands by "Switch Profile: Personal"
    And the user presses "Enter"
    Then the Workspace Hub overlay should be present
    And the Workspace Hub should show 1 workspace items

  Scenario: Deleting the last profile returns to the first-run welcome
    Given a profile "Work" with 0 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    And the user presses "d"
    And the user confirms the dialog
    Then the Workspace Hub overlay should be present
    And the first-run welcome card should be visible
