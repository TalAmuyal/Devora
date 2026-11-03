Feature: File preview pane
  An external tool (debi preview) asks Ember via the IPC HTTP server to render a Markdown or HTML file in a pane beside the terminal of a specific session.

  Background:
    Given Ember is running
    And a session exists

  Scenario: Stacking opens an additional pane
    When a Markdown file is previewed in the active session
    And another Markdown file is previewed in the active session with --stack
    Then the active session should have 2 preview panes
