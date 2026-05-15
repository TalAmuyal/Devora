Feature: Session management
  Users create, switch between, and close terminal sessions.
  Each session is a tab with its own PTY.

  Background:
    Given Ember is running

  Scenario: Create a new session
    When a new session is created with title "test-session"
    Then there should be 1 session
    And the active session title should be "test-session"

  Scenario: Switch between sessions
    Given 2 sessions exist
    When the user switches to the previous session
    Then the first session should be active

  Scenario: Close a session
    Given 2 sessions exist
    When the active session is closed
    Then there should be 1 session

  Scenario: Terminal receives PTY output
    When a new session is created
    And "echo EMBER_BDD_TEST" is typed in the terminal
    Then the terminal should contain "EMBER_BDD_TEST"
