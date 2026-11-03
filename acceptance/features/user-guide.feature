Feature: User Guide shortcut
  The User Guide is available even when a dialog is open.

  Background:
    Given Devora is running with no sessions open

  Scenario: F1 opens the User Guide over a dialog
    Given a profile "Work" with repos "repo-alpha"
    And the Workspace Hub is open
    And the user opens the New Task form
    When the user presses F1
    Then the User Guide should be visible
    And the User Guide should be stacked above the dialog
    When the user presses F1
    Then the User Guide should be open only once
    When the user presses "Escape"
    Then the User Guide should be closed and the dialog restored
