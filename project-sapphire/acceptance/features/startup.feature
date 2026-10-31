Feature: Startup and keyboard focus
  Sapphire opens on the Workspace Hub when it has no tabs to restore, as Ember always does, and on the restored tabs otherwise; the active session's terminal holds the keyboard focus.

  Scenario: With no tabs to restore, Sapphire starts on the Workspace Hub
    Given Sapphire is running
    Then there should be 0 sessions
    And the Workspace Hub should be visible
    And the active category should be "Active"

  Scenario: With tabs to restore, Sapphire starts on them without the Workspace Hub
    Given Devora is running with no sessions open
    And 2 sessions exist
    When Sapphire is quit and relaunched
    Then the same sessions should be open, with the same one active
    And the Workspace Hub should not be visible
    And the active session's terminal should have the keyboard focus
    When "echo SAPPHIRE_RESTORED" is typed in the terminal
    Then the terminal should contain "SAPPHIRE_RESTORED"

  Scenario: The keyboard focus follows the active session
    Given Devora is running with no sessions open
    When a new session is created
    Then the active session's terminal should have the keyboard focus
    When a new session is created
    And the user switches to the previous session
    Then the active session's terminal should have the keyboard focus
