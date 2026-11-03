@preview
Feature: File preview pane
  An external tool (debi preview) asks Devora via the IPC HTTP server to render a Markdown or HTML file in a pane beside the terminal of a specific session.

  Background:
    Given Devora is running with no sessions open
    And a session exists

  Scenario: Previewing a Markdown file opens a rendered pane
    When a Markdown file is previewed in the active session
    Then the active session should have 1 preview pane
    And the preview pane should render the Markdown heading "First Doc"

  Scenario: A Mermaid code block in Markdown renders as a themed diagram
    When a Markdown file with a Mermaid diagram is previewed in the active session
    Then the preview pane should render a Mermaid diagram containing "Client"
    And the Mermaid diagram nodes should use a themed fill color

  Scenario: A second file replaces the current preview by default
    When a Markdown file is previewed in the active session
    And another Markdown file is previewed in the active session
    Then the active session should have 1 preview pane

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

  Scenario: Previewing an HTML file displays sandboxed content
    When an HTML file is previewed in the active session
    Then the active session should have 1 preview pane
    And the preview pane should display sandboxed HTML
