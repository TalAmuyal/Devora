Feature: Theme loading
  Ember loads a terminal color theme from a kitty configuration file
  and applies it as CSS custom properties on the document root.

  Scenario: Theme CSS properties are applied on startup
    Given Ember is running
    Then the document root should have CSS property "--color-terminal-bg"
    And the document root should have CSS property "--color-terminal-fg"
