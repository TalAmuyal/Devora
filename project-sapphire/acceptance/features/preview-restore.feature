@preview
Feature: Durable preview panes
  Background:
    Given Devora is running with no sessions open
    And a session exists

  Scenario: Refresh rereads the same preview file
    When a Markdown file is previewed in the active session
    Then the preview pane should render the Markdown heading "First Doc"
    When the preview file is changed to heading "Updated Doc"
    And the same Markdown file is previewed again in the active session
    Then the active session should have 1 preview pane
    And the preview pane should render the Markdown heading "Updated Doc"

  Scenario: A preview survives a restart and rereads its file
    When a Markdown file is previewed in the active session
    Then the preview pane should render the Markdown heading "First Doc"
    When the preview file is changed to heading "Restored Doc"
    And Sapphire is quit and relaunched
    Then the active session should have 1 preview pane
    And the preview pane should render the Markdown heading "Restored Doc"

  Scenario: A missing restored file stays visible as an error
    When a Markdown file is previewed in the active session
    Then the preview pane should render the Markdown heading "First Doc"
    When the preview file is deleted
    And Sapphire is quit and relaunched
    Then the active session should have 1 preview pane
    And the preview pane should show a file error

  Scenario: A preview refresh in a background tab preserves the active tab
    When a Markdown file is previewed in the active session
    Then the preview pane should render the Markdown heading "First Doc"
    When a new session is created
    And the last preview file is sent to the first session
    Then the first session should have 1 preview pane
    And the active session should have 0 preview panes
    And the second session should be active
