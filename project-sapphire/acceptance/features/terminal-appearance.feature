Feature: Terminals follow the appearance
  The terminals draw in the palette chosen in User Defaults, its light or dark variant after the app's effective
  appearance, both live; a program that asked to be told of color-scheme changes (mode 2031) is told, and reads the
  new background when it asks (OSC 11).

  Scenario: A program in the terminal is told when the appearance switches
    Given a profile "Work" with 1 active workspaces
    And a session exists
    When a program listening for color-scheme changes runs in the terminal
    And the Workspace Hub is open
    And the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    # Dark first, whatever the Mac's own appearance, so that each switch below is a change.
    And the user switches "appearance" to "Dark"
    And the user switches "appearance" to "Light"
    Then the program should last have been told "light", with a "light" background
    When the user switches "appearance" to "Dark"
    Then the program should last have been told "dark", with a "dark" background

  Scenario: The terminal colors are chosen in User Defaults and apply at once
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    Then the terminals should be drawn on "#fbf3db" in light and "#103c48" in dark
    When the user switches "terminal.palette" to "Gruvbox"
    Then the global config should have "terminal.palette" set to "gruvbox"
    And the terminals should be drawn on "#fbf1c7" in light and "#282828" in dark
    When the user switches "terminal.palette" to "Catppuccin"
    Then the terminals should be drawn on "#eff1f5" in light and "#24273a" in dark
    When the user switches "terminal.palette" to "Selenized"
    Then the global config should not have "terminal.palette"
    When the user opens the "Work" settings detail
    Then the setting "terminal.palette" should not be shown
