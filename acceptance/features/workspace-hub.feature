Feature: Workspace Hub
  The Workspace Hub shows workspaces for the active profile.
  Users navigate with keyboard shortcuts, filter, and open workspaces.

  Background:
    Given Devora is running with no sessions open

  Scenario: Hub shows workspaces from profile
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    Then the Workspace Hub should show 3 workspace items

  Scenario: Navigate workspaces with j/k
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user presses "j" 2 times
    Then the focused workspace should be "ws-3"
    When the user presses "k"
    Then the focused workspace should be "ws-2"

  Scenario: Open workspace with Enter
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user presses "j"
    And the user presses "Enter"
    Then the Workspace Hub should not be visible
    And there should be 1 session

  Scenario: Close hub with q
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "q"
    Then the Workspace Hub should not be visible

  Scenario: Filter workspaces
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user filters workspaces by "Task 2"
    Then the Workspace Hub should show 1 workspace items

  Scenario: Switch category tabs
    Given a profile "Work" with 2 active and 1 inactive workspaces
    And the Workspace Hub is open
    Then the Workspace Hub should show 2 workspace items
    When the user presses "3"
    Then the Workspace Hub should show 3 workspace items
    And the active category should be "All"
    When the user presses "2"
    Then the Workspace Hub should show 1 workspace items
    And the active category should be "Inactive"

  Scenario: The detail panel shows the focused workspace's repo status
    Given a profile "Work" with 2 active workspaces with worktrees
    And the Workspace Hub is open
    Then the detail panel should show repo status

  Scenario: Toggle cheatsheet with ?
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "?"
    Then the cheatsheet should be visible
    When the user presses "?"
    Then the cheatsheet should not be visible

  Scenario: Hub keys are inert while the cheatsheet is open
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "?"
    Then the cheatsheet should be visible
    When the user presses "n"
    Then the cheatsheet should be visible
    And the New Task dialog should not be open

  Scenario: Scroll the cheatsheet with Vim keys
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "?"
    Then the cheatsheet should be visible
    And the cheatsheet should overflow its viewport
    When the user presses Ctrl+D
    Then the cheatsheet scroll offset should be greater than 0
    When the user presses "g" 2 times
    Then the cheatsheet scroll offset should be 0
    When the user presses "G"
    Then the cheatsheet scroll offset should be greater than 0

  # Sapphire's test bridge can't see what SwiftUI draws; its XCUITests check the same (`WorkspaceHubUITests`).
  @ember-only
  Scenario: Hub shell renders immediately on load
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is loading
    Then the hub header should be visible
    And the hub legend should be visible
    And the hub search bar should be visible

  Scenario: Hub is navigable immediately after opening
    Given a profile "Work" with 3 active workspaces
    And the Workspace Hub is open
    When the user presses "j"
    Then the focused workspace should be "ws-2"
    When the user presses "k"
    Then the focused workspace should be "ws-1"

  Scenario: Refresh picks up a newly added workspace
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When a second active workspace is added to profile "Work"
    And the user presses "R"
    Then the Workspace Hub should show 2 workspace items

  Scenario: Refresh preserves the current category filter
    Given a profile "Work" with 2 active and 1 inactive workspaces
    And the Workspace Hub is open
    And the category filter is set to "All"
    When the user presses "R"
    Then the category filter should still be "All"
    And the Workspace Hub should show 3 workspace items

  Scenario: Refresh shows progress then success toasts
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "R"
    Then the refresh toast should read "Refreshing"
    And the refresh toast should read "Refreshed successfully"
    And the refresh toast should disappear

  Scenario: Pressing refresh again restarts cleanly without leaving stray toasts
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses "R"
    And the user presses "R"
    Then the refresh toast should read "Refreshed successfully"
    And the refresh toast should disappear

  # Ctrl+←/→ is blocked while a page is open, so it neither switches the terminal tab nor steals focus from the hub.
  Scenario: Ctrl+Right with the hub open does not switch tabs and the hub stays keyboard-operable
    Given a profile "Work" with 3 active workspaces
    And 2 sessions exist
    And the Workspace Hub is open
    When the user presses Ctrl+Right
    Then the second session should be active
    And the Workspace Hub overlay should be present
    When the user presses "j"
    Then the focused workspace should be "ws-2"

  Scenario: Ctrl+S opens the Workspace Hub but never closes it
    Given a profile "Work" with 1 active workspaces
    And the Workspace Hub is open
    When the user presses Ctrl+S
    Then the Workspace Hub should be visible
