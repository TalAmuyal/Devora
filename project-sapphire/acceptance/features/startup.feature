Feature: Startup and keyboard focus
  Sapphire opens with one shell session, and the active session's terminal holds the keyboard focus.

  Scenario: Sapphire starts with a shell session
    Given Sapphire is running
    Then there should be 1 session
    And the active session's terminal should have the keyboard focus
    When "echo SAPPHIRE_STARTED" is typed in the terminal
    Then the terminal should contain "SAPPHIRE_STARTED"

  Scenario: The keyboard focus follows the active session
    Given Sapphire is running
    When a new session is created
    Then the active session's terminal should have the keyboard focus
    When the user switches to the previous session
    Then the active session's terminal should have the keyboard focus
