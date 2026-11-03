@preview
Feature: Isolated HTML previews
  Background:
    Given Devora is running with no sessions open
    And a session exists

  Scenario: Untrusted HTML cannot run scripts load resources or navigate
    When an HTML file containing scripts and resource probes is previewed
    And HTML links popups and forms attempt to leave the preview
    Then the HTML sandbox should block scripts resources and navigation

  Scenario: Refresh rereads HTML without changing the pane
    When an HTML file is previewed in the active session
    Then the preview pane should render the HTML heading "Hello HTML"
    When the preview file is changed to heading "Updated HTML"
    And the same preview file is previewed again in the active session
    Then the active session should have 1 preview pane
    And the preview pane should render the HTML heading "Updated HTML"

  Scenario: HTML restores by rereading its file
    When an HTML file is previewed in the active session
    Then the preview pane should display sandboxed HTML
    When the preview file is changed to heading "Restored HTML"
    And Sapphire is quit and relaunched
    Then the preview pane should render the HTML heading "Restored HTML"

  Scenario: A missing restored HTML file shows an in-pane error
    When an HTML file is previewed in the active session
    Then the preview pane should display sandboxed HTML
    When the preview file is deleted
    And Sapphire is quit and relaunched
    Then the preview pane should show a file error

  Scenario: Switching file formats rebuilds the renderer
    When a Markdown file is previewed in the active session
    Then the preview pane should render the HTML heading "First Doc"
    When an HTML file is previewed in the active session
    Then the preview pane should display sandboxed HTML
    When another Markdown file is previewed in the active session
    Then the preview pane should render the HTML heading "Second Doc"
