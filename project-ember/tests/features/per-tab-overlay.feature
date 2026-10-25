@task-creation
Feature: Per-tab surfaces
  A surface on a session's own layer stack (here, task-creation progress) covers only its tab:
  it is shown only while that tab is active, takes q even while the terminal beneath it holds
  focus, and never blocks window-level shortcuts such as tab switching.

  Background:
    Given Ember is running
    And an origin-backed profile "Work" with repo "test-repo"
    And the prepare-command sleeps
    And a session exists
    And the Workspace Hub is open
    When the user creates a task "Long setup" selecting repo "test-repo"
    Then the second session should have a panel overlay

  Scenario: A tab surface is visible only while its tab is active
    Then the panel overlay should be visible
    When the user switches to the first session
    Then the panel overlay should not be visible
    When the user switches to the second session
    Then the panel overlay should be visible
    When the user cancels the task creation once it is preparing
    Then the task-creation tab should close

  Scenario: q reaches a tab surface while the terminal beneath it holds focus
    Given the active session's terminal holds focus
    When the user presses "q" without first taking focus
    Then the task-creation tab should close

  Scenario: A tab surface does not freeze tab switching
    When the user presses Ctrl+Right
    Then the first session should be active
    When the user switches to the second session
    And the user cancels the task creation once it is preparing
    Then the task-creation tab should close
