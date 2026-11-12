@repo-updates
Feature: Ember repository updates and Global Pause
  Scenario: Repository update settings default off and retain numeric intervals
    When Ember User Defaults repository settings are opened
    Then Ember periodic repo updates default to off
    When Ember periodic repo updates are set to "On"
    And Ember repo update interval is set to 15 minutes
    Then Ember updater config stores enabled true and interval 15
    When Ember periodic repo updates are set to "Off"

  Scenario: Manual updates while disabled respect the saved global hold
    Given an Ember parent clone with an advanced origin
    When the Ember palette runs "Pause Devora"
    Then the Ember Paused hint is visible
    When the Ember palette runs "Run Repo Updates"
    Then the Ember parent clone keeps its original commit
    When the Ember palette runs "Resume Devora"
    Then the Ember parent clone reaches its advanced origin
    And the Ember Paused hint is hidden

  Scenario: The global hold survives a real app restart
    When the Ember palette runs "Pause Devora"
    Then the Ember Paused hint is visible
    When Ember restarts with its saved state
    Then the Ember Paused hint is visible
    When the Ember palette runs "Resume Devora"
    Then the Ember Paused hint is hidden
