Feature: Failure capture
  When a step fails, the harness saves a screenshot and the active terminal's text, so a CI failure can be inspected from its artifacts without reproducing it.

  @screen-capture
  Scenario: A failure capture saves a screenshot and the terminal text
    Given Ember is running
    And a session exists
    When "echo failure-capture-probe" is typed in the terminal
    Then the terminal should contain "failure-capture-probe"
    When a failure is captured
    Then the capture should include a screenshot
    And the capture should include the terminal text "failure-capture-probe"
