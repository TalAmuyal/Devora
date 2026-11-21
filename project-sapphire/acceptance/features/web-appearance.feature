Feature: Web panes follow the appearance
  The User Guide and the Markdown previews draw in the system's colors for the app's effective appearance, on a
  document's background, and follow a switch live.

  Scenario: The User Guide follows a switch of appearance while it's open
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "User Defaults" settings detail
    And the user presses F1
    Then the User Guide should be visible
    When the user switches "appearance" to "Light"
    Then the User Guide's page should follow the app's appearance
    When the user switches "appearance" to "Dark"
    Then the User Guide's page should follow the app's appearance
    When the user switches "appearance" to "Light"
    Then the User Guide's page should follow the app's appearance
