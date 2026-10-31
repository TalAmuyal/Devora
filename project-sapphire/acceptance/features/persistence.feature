Feature: Tabs survive a restart
  Sapphire keeps its tabs in its app state, so they come back after a quit, a crash or a kill; each tab's terminal starts afresh.

  Scenario: Tabs survive a quit and relaunch
    Given a profile "work" with 2 active workspaces
    And a session exists
    And "echo BEFORE_RESTART" is typed in the terminal
    And the user opens the workspace "ws-1"
    And a new session is created
    And the user switches to the previous session
    When Sapphire is quit and relaunched
    Then the same sessions should be open, with the same one active
    When the user switches to the previous session
    Then the terminal should not contain "BEFORE_RESTART"
    When "echo AFTER_RESTART" is typed in the terminal
    Then the terminal should contain "AFTER_RESTART"

  Scenario: Tabs survive a kill -9
    Given 2 sessions exist
    When Sapphire is killed and relaunched
    Then the same sessions should be open, with the same one active

  Scenario: Closing every tab before quitting starts the next launch on the Workspace Hub
    Given Devora is running with no sessions open
    And a session exists
    When the active session is closed
    And Sapphire is quit and relaunched
    Then there should be 0 sessions
    And the Workspace Hub should be visible

  Scenario: A workspace still being created when Sapphire stopped comes back as failed
    Given Sapphire stopped while a session was creating its workspace
    When Sapphire is launched
    Then that session should show that its workspace creation was interrupted

  Scenario: An unreadable app state is kept aside and Sapphire starts fresh
    Given a new session is created
    And Sapphire has quit
    And the app state is corrupt
    When Sapphire is launched
    Then Sapphire should report that the app state can't be opened
    When the user chooses to start fresh
    Then there should be 0 sessions
    And the Workspace Hub should be visible
    And the corrupt app state should be kept beside the new one

  Scenario: A second Sapphire on the same app state is turned away
    Given a new session is created
    When a second Sapphire is launched on the same app state
    Then the second Sapphire should report that another Sapphire is using the app state
    When the second Sapphire quits
    And Sapphire is quit and relaunched
    Then the same sessions should be open, with the same one active
