@command-palette
Feature: Command Palette
  A rapid double-tap of Shift opens a searchable Command Palette of actions.
  The Command Palette and the Workspace Hub are mutually exclusive overlays.

  Background:
    Given Ember is running

  Scenario: Open the Command Palette with shift+shift
    Given no overlay is open
    When the user opens the Command Palette via shift+shift
    Then the Command Palette should be visible
    And the Command Palette should show at least 2 commands

  Scenario: Opening the Command Palette focuses the search field
    Given a session exists
    And the Command Palette is open
    Then the search field should be focused

  Scenario: Reopen with shift+shift after dismissing it
    Given no overlay is open
    When the user opens the Command Palette via shift+shift
    Then the Command Palette should be visible
    When the user presses "Escape"
    Then the Command Palette should not be visible
    When the user opens the Command Palette via shift+shift
    Then the Command Palette should be visible

  Scenario: Navigate with j and run the New Shell command
    Given the Command Palette is open
    When the user presses "j"
    And the user presses "Enter"
    Then the Command Palette should not be visible
    And there should be 1 session

  Scenario: Keyboard navigation works when a terminal session was focused
    Given a session exists
    And the Command Palette is open
    When the user presses "j"
    Then the selected command should be "New Shell"

  Scenario: Close the current session from the palette
    Given a session exists
    And the Command Palette is open
    When the user filters commands by "close"
    Then the Command Palette should show 1 command
    When the user presses "Enter"
    Then the Command Palette should not be visible
    And there should be 0 sessions

  Scenario: Closing with no session is a clean no-op
    Given the Command Palette is open
    When the user filters commands by "close"
    Then the Command Palette should show 1 command
    When the user presses "Enter"
    Then the Command Palette should not be visible
    And there should be 0 sessions

  Scenario: Run the Workspace Hub command
    Given a profile "Work" with 1 active workspaces
    And the Command Palette is open
    When the user presses "Enter"
    Then the Command Palette should not be visible
    And the Workspace Hub overlay should be present

  # Uses a raw keypress (no test-only blur) so it genuinely proves the hub — opened from the palette while a terminal session held focus — has keyboard focus, rather than relying on a click or the harness blurring the terminal.
  Scenario: The Workspace Hub opened from the palette has keyboard focus
    Given a profile "Work" with 1 active workspaces
    And a session exists
    And the Command Palette is open
    When the user presses "Enter"
    Then the Workspace Hub overlay should be present
    When the user presses "q" without first taking focus
    Then the Workspace Hub overlay should not be present

  Scenario: Escape in the focused search field closes the Command Palette
    Given the Command Palette is open
    When the user presses Escape in the search field
    Then the Command Palette should not be visible

  # The palette is type-first: with the search field focused, q is a search character, not a close shortcut (a real user can no longer close with q).
  Scenario: Pressing q does not close the Command Palette
    Given the Command Palette is open
    And the search field should be focused
    When the user presses "q" without first taking focus
    Then the Command Palette should be visible

  Scenario: Filter commands
    Given the Command Palette is open
    When the user filters commands by "shell"
    Then the Command Palette should show 1 command
    And the selected command should be "New Shell"

  Scenario: An open Workspace Hub blocks the Command Palette
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user opens the Command Palette via shift+shift
    Then the Command Palette should not be visible
    And the Workspace Hub overlay should be present

  Scenario: An open Command Palette blocks the Workspace Hub
    Given the Command Palette is open
    When the user presses Ctrl+S
    Then the Workspace Hub overlay should not be present
    And the Command Palette should be visible
