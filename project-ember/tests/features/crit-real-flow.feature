@real-crit
Feature: Crit end-to-end flow
  The crit wrapper launches original-crit, starts a local server,
  and signals Ember to open a panel overlay with the code review UI.

  Background:
    Given A repo named "repo-1" exists
    And Ember is running
    And a task named "task-1" is created with repos: "repo-1"

  Scenario: Crit overlay opens with review UI and user approves
    When a file is modified in the workspace
    And the user runs "crit" in the terminal
    Then the active session should have a panel overlay
    And the overlay should display a "Crit Review" header
    And the overlay should contain an iframe loading a Crit URL
    And the Crit review UI should be fully loaded
    When the user presses the Approve button in the Crit overlay
    Then the active session should not have a panel overlay
