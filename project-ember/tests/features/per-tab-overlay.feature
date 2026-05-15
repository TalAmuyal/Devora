Feature: Per-tab panel overlay
  When a panel overlay opens, it is displayed only when its owning tab is active.

  Background:
    Given Ember is running
    And 2 sessions exist

  Scenario: Current-tab overlay is displayed immediately
    When an external tool sends a crit-open request for the active session with URL "http://example.com/review"
    Then the active session should have a panel overlay
    And the panel overlay should be visible

  Scenario: Background-tab overlay is hidden until tab is activated
    When an external tool sends a crit-open request for the first session with URL "http://example.com/review"
    Then the first session should have a panel overlay
    But the panel overlay should not be visible
    When the user switches to the first session
    Then the panel overlay should be visible

  @real-claude
  Scenario: Claude Code hook overlay appears only on owning tab
    Given the first session has a workspace with a PostToolUse crit hook
    And the user switches to the second session
    When Claude Code triggers a tool use in the first session
    Then the first session should have a panel overlay
    But the panel overlay should not be visible
    When the user switches to the first session
    Then the panel overlay should be visible
