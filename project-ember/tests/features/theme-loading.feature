Feature: Theme loading
  Ember loads the terminal color theme bundled in the app
  and applies it as CSS custom properties on the document root.

  Scenario: Theme CSS properties are applied on startup
    Given Ember is running
    Then the document root should have the theme CSS property "--color-terminal-bg"
    And the document root should have the theme CSS property "--color-terminal-fg"
