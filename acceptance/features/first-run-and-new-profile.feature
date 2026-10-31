Feature: First-run welcome and new profiles
  With no profiles, the Workspace Hub shows a welcome card with the profile form and can't be closed.
  The profile dropdown's "New Profile…" opens the same form, which validates the root path as a new profile, an existing one, or an invalid path.

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

  Scenario: Registering an existing profile directory from the profile dropdown
    Given a profile "Work" with 1 active workspaces
    And an unregistered initialized profile directory "Legacy"
    And the Workspace Hub is open
    When the user clicks the profile dropdown
    And the user clicks the "New Profile…" dropdown action
    And the user enters path "Legacy" under the fixture root in the profile form
    Then the profile form should detect the existing profile "Legacy"
    And the profile form submit button should read "Register Profile"
    When the user submits the profile form
    Then the profile dropdown should be labeled "Legacy"
    And the global config should list 2 profiles
