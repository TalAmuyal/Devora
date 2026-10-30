Feature: Key routing through the layers
  Keys go first to the app-wide shortcuts, then to the window's surfaces, then to the active session's, whose bottom layer is its terminal.
  A window surface stands in here for the hubs and the palette, which Sapphire doesn't have yet.

  Background:
    Given Devora is running with no sessions open

  Scenario: Keys no shortcut claims reach the terminal, Ctrl+C included
    Given a session exists
    When "echo START_$((6*7)); sleep 30; echo NOT_$((6*7))" is typed in the terminal
    Then the terminal should contain "START_42"
    When the user presses Ctrl+C
    And "echo DONE_$((6*7))" is typed in the terminal
    Then the terminal should contain "DONE_42"

  Scenario: With a window surface open, session shortcuts reach nothing, and the font size still changes
    Given 2 sessions exist
    And a window surface is open
    Then no session should have the keyboard focus
    When the user presses Ctrl+Left
    And the user presses Ctrl+Shift+S
    And the user presses Ctrl+3
    Then the terminal font size should be 26
    And there should be 2 sessions
    And the second session should be active
    And no session should have the keyboard focus

  Scenario: Closing the window surface gives the keyboard focus back to the active session's terminal
    Given 2 sessions exist
    And a window surface is open
    When the user presses "q"
    Then no window surface should be open
    And the active session's terminal should have the keyboard focus

  Scenario: The keyboard focus follows Ctrl+Right to the next session
    Given 2 sessions exist
    When the user presses Ctrl+Right
    Then the first session should be active
    And the active session's terminal should have the keyboard focus
