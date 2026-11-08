Feature: Parent clone updates
  User Defaults controls the periodic worker, and a manual round respects Global Pause.

  Background:
    Given an origin-backed profile "Work" with repo "api"
    And the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible

  Scenario: Periodic settings are global and keep their numeric type
    When the user opens the "User Defaults" settings detail
    Then the setting "repo-update.enabled" should show the hint "→ on"
    And the setting "repo-update.interval-minutes" should show the hint "→ 60"
    When the user switches "repo-update.enabled" to "Off"
    And the user sets "repo-update.interval-minutes" to "15"
    Then the updater config stores interval 15 and enabled false
    When the user opens the "Work" settings detail
    Then the setting "repo-update.enabled" should not be shown
    And the setting "repo-update.interval-minutes" should not be shown

  Scenario: A manual round runs while periodic updates are off and waits for Global Pause
    When the user opens the "User Defaults" settings detail
    And the user switches "repo-update.enabled" to "Off"
    Given the parent clone "api" is detached and its current commit is recorded
    And the upstream of parent clone "api" advances without changing its checkout
    When the user globally pauses Devora
    And the user requests a repo update round from the palette
    Then the manual repo update request is logged
    And the parent clone "api" is still at its recorded commit
    When the user toggles Global Pause from the palette
    Then the parent clone "api" reaches the new origin commit
    Given the origin of repo "api" is gone
    When the user requests a repo update round from the palette
    Then the repo update failure is logged without an error banner
