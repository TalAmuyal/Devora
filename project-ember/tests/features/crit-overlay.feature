Feature: Crit overlay integration
  External tools communicate with Ember via the IPC HTTP server
  to trigger panel overlays on specific session tabs.

  Background:
    Given Ember is running
    And a session exists

  Scenario: IPC request triggers a panel overlay
    When an external tool sends a crit-open request for the active session with URL "http://example.com/review"
    Then the active session should have a panel overlay

  Scenario: Crit done request closes the overlay
    Given the active session has a crit overlay
    When an external tool sends a crit-done request for the active session
    Then the active session should not have a panel overlay

  Scenario: q dismisses the crit overlay while the terminal holds focus
    Given no overlay is open
    And the active session has a crit overlay
    And the active session's terminal holds focus
    When the user presses "q" without first taking focus
    Then the active session should not have a panel overlay
