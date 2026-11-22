Feature: Terminals follow the appearance
  The terminals draw in the palette chosen in User Defaults, its light or dark variant after the app's effective
  appearance, both live.

  # A program running in a terminal isn't told of a switch (mode 2031); its scenario returns with that follow-up.

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

  # A new terminal keeps what Sapphire gives it (Devora's environment, its Default app, its workspace's directory)
  # whatever the scheme when it's made: in Dark, libghostty once dropped the environment and the command.
  Scenario: A terminal made at launch in Dark keeps Devora's environment, its default app and its directory
    Given a profile "Work" with 1 active workspaces
    And the profile "Work" runs the stand-in as its default app
    And the global config sets Appearance to "dark"
    When Sapphire is quit and relaunched
    And the user opens the profile's first workspace
    Then the default app should see Devora's environment and start in its workspace

  Scenario Outline: A terminal made after switching <from> to <to> keeps Devora's environment, its default app and its directory
    Given a profile "Work" with 1 active workspaces
    And the profile "Work" runs the stand-in as its default app
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    And the user switches "appearance" to "<from>"
    And the user switches "appearance" to "<to>"
    And the user opens the profile's first workspace
    Then the default app should see Devora's environment and start in its workspace

    Examples:
      | from  | to    |
      | Light | Dark  |
      | Dark  | Light |
