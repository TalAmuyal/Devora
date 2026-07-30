Feature: App-wide keyboard shortcuts
  App-wide shortcuts are handled before any surface sees the key, so each one carries its own availability.
  Font size and the User Guide are never unavailable; the shortcuts that open a window surface are inert
  while one is already open, and never fall through to the surface that blocked them.

  Background:
    Given Ember is running

  Scenario: Font size changes inside the Workspace Hub, whose own keys claim bare digits
    Given a profile "Work" with 2 active and 1 inactive workspaces
    And the Workspace Hub is open
    When the user presses Ctrl+3
    Then the UI font size should be "26px"
    And the active category should be "Active"

  Scenario: Font size changes while a dialog is open
    Given a profile "Work" with repos "repo-alpha"
    And the Workspace Hub is open
    And the user opens the New Task form
    When the user presses Ctrl+1
    Then the UI font size should be "12px"

  Scenario: F1 opens the User Guide over a dialog
    Given a profile "Work" with repos "repo-alpha"
    And the Workspace Hub is open
    And the user opens the New Task form
    When the user presses F1
    Then the User Guide should be visible
    And the User Guide should be stacked above the dialog

  Scenario: Ctrl+S opens the Workspace Hub but never closes it
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses Ctrl+S
    Then the Workspace Hub should be visible
