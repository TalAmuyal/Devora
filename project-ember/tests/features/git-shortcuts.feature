@git-shortcuts
Feature: Debi git-shortcut shims Devora-Ember session shells expose Debi's git shortcuts as bare commands (e.g. `gcl` runs `debi gcl`) via PATH shims, so they work in any shell — including non-zsh sub-shells.
  The feature is opt-out via the `terminal.git-shortcuts` config key.

  Background:
    Given Ember is running

  Scenario: Git shortcuts are available as bare commands
    When a new session is created
    And "command -v gcl | grep -q git-shortcuts && echo SHIM_FOUND" is typed in the terminal
    Then the terminal should contain "SHIM_FOUND"

  Scenario: Git shortcuts work in a non-zsh sub-shell
    When a new session is created
    And "bash -lc 'command -v gcl' | grep -q git-shortcuts && echo SHIM_IN_SUBSHELL" is typed in the terminal
    Then the terminal should contain "SHIM_IN_SUBSHELL"

  Scenario: Git shortcuts can be disabled via config
    Given the global config disables git shortcuts
    When a new session is created
    And "command -v gcl || echo SHIM_ABSENT" is typed in the terminal
    Then the terminal should contain "SHIM_ABSENT"
