@git-shortcuts @bundled-tools
Feature: Debi git-shortcut shims
  Session shells expose Debi's git shortcuts as bare commands (e.g. `gcl` runs `debi gcl`) via PATH shims, so they work in any shell, including non-zsh sub-shells.
  The feature is opt-out via the `terminal.git-shortcuts` config key.

  Background:
    Given Devora is running with no sessions open

  Scenario: Git shortcuts are available as bare commands
    When a new session is created
    And "command -v gcl | grep -q git-shortcuts && printf 'SHIM_%s\n' FOUND" is typed in the terminal
    Then the terminal should contain "SHIM_FOUND"

  Scenario: Git shortcuts work in a non-zsh sub-shell
    When a new session is created
    And "bash -lc 'command -v gcl' | grep -q git-shortcuts && printf 'SHIM_%s\n' IN_SUBSHELL" is typed in the terminal
    Then the terminal should contain "SHIM_IN_SUBSHELL"

  Scenario: Git shortcuts can be disabled via config
    Given the global config disables git shortcuts
    When a new session is created
    And "command -v gcl || printf 'SHIM_%s\n' ABSENT" is typed in the terminal
    Then the terminal should contain "SHIM_ABSENT"
