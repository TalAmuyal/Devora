@preview
Feature: Durable stacked preview layout
  Background:
    Given Devora is running with no sessions open
    And a session exists

  Scenario: Stacked order and sizes survive refresh and restart
    When a Markdown file is previewed in the active session
    And another Markdown file is previewed in the active session with --stack
    Then the active session should have 2 preview panes
    When the first preview is resized to 410 points
    And the first preview file is previewed again without --stack
    Then the preview headings should be "First Doc" and "Stacked Doc" in order
    When Sapphire is quit and relaunched
    Then the active session should have 2 preview panes
    And the first preview width should be 410 points
    And the preview headings should be "First Doc" and "Stacked Doc" in order
    When the preview pane close button is clicked
    Then the active session should have 1 preview pane
    And the preview pane should render the Markdown heading "Stacked Doc"
