Feature: File preview pane
  An external tool (debi preview) asks Ember via the IPC HTTP server to render a Markdown or HTML file in a pane beside the terminal of a specific session.

  Background:
    Given Ember is running
    And a session exists

  Scenario: Previewing a Markdown file opens a rendered pane
    When a Markdown file is previewed in the active session
    Then the active session should have 1 preview pane
    And the preview pane should render the Markdown heading "First Doc"

  Scenario: Previewing an HTML file uses a sandboxed iframe
    When an HTML file is previewed in the active session
    Then the active session should have 1 preview pane
    And the preview pane should contain a sandboxed iframe

  Scenario: A second file replaces the current preview by default
    When a Markdown file is previewed in the active session
    And another Markdown file is previewed in the active session
    Then the active session should have 1 preview pane

  Scenario: Stacking opens an additional pane
    When a Markdown file is previewed in the active session
    And another Markdown file is previewed in the active session with --stack
    Then the active session should have 2 preview panes

  Scenario: Re-previewing the same file refreshes its pane
    When a Markdown file is previewed in the active session
    And the same Markdown file is previewed again in the active session
    Then the active session should have 1 preview pane

  Scenario: Closing a preview pane removes it
    When a Markdown file is previewed in the active session
    And the preview pane close button is clicked
    Then the active session should have 0 preview panes

  Scenario: A preview for an unknown session is a no-op
    When a Markdown file is previewed for an unknown session
    Then the active session should have 0 preview panes
