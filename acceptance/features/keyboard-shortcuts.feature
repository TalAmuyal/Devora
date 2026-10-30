Feature: App-wide keyboard shortcuts for sessions and the font size
  App-wide shortcuts run before any surface or terminal sees the key.
  Switching sessions wraps around, moving one doesn't, and one font size applies to every terminal.

  Background:
    Given Devora is running with no sessions open

  Scenario: Ctrl+Right and Ctrl+Left switch sessions, wrapping around
    Given 3 sessions exist
    When the user presses Ctrl+Right
    Then the first session should be active
    When the user presses Ctrl+Left
    Then the third session should be active
    When the user presses Ctrl+Left
    Then the second session should be active

  Scenario: Ctrl+Shift+Left and Ctrl+Shift+Right move the active session, without wrapping
    Given 3 sessions exist
    When the user presses Ctrl+Shift+Right
    Then the sessions should be in the order 1, 2, 3
    When the user presses Ctrl+Shift+Left
    Then the sessions should be in the order 1, 3, 2
    When the user presses Ctrl+Shift+Left
    And the user presses Ctrl+Shift+Left
    Then the sessions should be in the order 3, 1, 2
    And the first session should be active

  Scenario: Ctrl+Shift+S opens a new shell session
    Given a session exists
    When the user presses Ctrl+Shift+S
    Then there should be 2 sessions
    And the second session should be active

  Scenario: The font size shortcuts set one size for the terminals, within 8 to 40
    Given a session exists
    When the user presses Ctrl+3
    Then the terminal font size should be 26
    When the user presses Ctrl+Shift+=
    Then the terminal font size should be 28
    When the user presses Ctrl+=
    Then the terminal font size should be 15
    When the user presses Ctrl+1
    And the user presses Ctrl+Shift+-
    And the user presses Ctrl+Shift+-
    And the user presses Ctrl+Shift+-
    Then the terminal font size should be 8
