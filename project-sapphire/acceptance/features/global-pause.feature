Feature: Global Pause
  Every agent reaches a graceful turn boundary, and the app remembers its hold.

  Background:
    Given an origin-backed profile "Work" with repo "api"
    And Devora is running with no sessions open
    And the Agent Hub is open
    And an agent "Echo" was created
    When the user opens the chat for "Echo"

  Scenario: A permission-held turn finishes and the palette releases the global hold
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    When the user globally pauses Devora
    Then Devora is globally paused
    And the agent "Echo" should be pausing
    When the user allows the agent permission with Command+Y
    Then the agent chat is paused
    And the agent "Echo" has a paused tab indicator
    When the user drafts the agent prompt "Keep my draft"
    And the user attempts to send during Global Pause
    Then the agent prompt draft is "Keep my draft"
    And the global pause send hint is shown
    When the user toggles Global Pause from the palette
    Then Devora is globally running
    And the agent "Echo" should be idle
    And the agent prompt draft is "Keep my draft"

  Scenario: Relaunch preserves the global hold and restores history without a CLI launch
    When the user shows the agent's system messages
    And the user sends the agent prompt "History before the hold"
    Then the agent "Echo" should be idle
    When the user drafts the agent prompt "After relaunch"
    And the user globally pauses Devora
    Then the agent chat is paused
    When the current agent CLI launch count is recorded
    And Sapphire is quit and relaunched
    Then Devora is globally paused
    And the agent chat is paused
    And the agent transcript contains "History before the hold"
    And the agent prompt draft is "After relaunch"
    And the agent CLI launch count is unchanged
    And the agent has not been told that Devora restarted
    When the user toggles Global Pause from the palette
    Then Devora is globally running
    And the agent has been told once that Devora restarted
    And the agent "Echo" should be idle
    And the agent prompt draft is "After relaunch"

  Scenario: Keep paused survives relaunch and can be reversed before Devora resumes
    When the user shows the agent's system messages
    And the user sends the agent prompt "Before the hold"
    Then the agent "Echo" should be idle
    When the user globally pauses Devora
    Then the agent chat is paused
    When the user keeps this agent paused through Global Pause
    And the current agent CLI launch count is recorded
    And Sapphire is quit and relaunched
    Then Devora is globally paused
    And the Keep paused switch is on
    And the agent CLI launch count is unchanged
    And the agent has not been told that Devora restarted
    When the user lets this agent resume with Devora
    And the user toggles Global Pause from the palette
    Then Devora is globally running
    And the agent has been told once that Devora restarted
    And the agent "Echo" should be idle
    And the pause widget shows "Pause"

  Scenario: Keep paused retains the individual hold after Devora resumes
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    When the user globally pauses Devora
    Then the pause widget shows "Pausing…"
    When the user keeps this agent paused through Global Pause
    And the user lets this agent resume with Devora
    And the user keeps this agent paused through Global Pause
    And the user allows the agent permission with Command+Y
    Then the agent chat is paused
    And the pause widget shows "Paused"
    When the user toggles Global Pause from the palette
    Then Devora is globally running
    And the agent chat is paused
    And the agent resume action is available
    When the user resumes the agent chat
    Then the agent "Echo" should be idle
    And the pause widget shows "Pause"

  Scenario: Quitting waits for an agent's question, then quits by itself once it's answered
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    When Sapphire is asked to quit
    Then Sapphire waits to quit for "Echo", which is "waiting for an answer"
    And Devora is globally paused
    And the toolbar shows "Pausing… (0 of 1)"
    When the user shows the agent "Echo" from the quit progress
    And the user allows the agent permission with Command+Y
    Then Sapphire quits by itself

  Scenario: Cancelling a waiting quit keeps the global hold
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    When Sapphire is asked to quit
    Then Sapphire waits to quit for "Echo", which is "waiting for an answer"
    When the user cancels the quit
    Then Sapphire is still running with no quit pending
    When the user allows the agent permission with Command+Y
    Then the toolbar shows "Paused"

  Scenario: Closing the window waits too, and Interrupt and quit cuts the turn short
    When the user sends the agent prompt "stream"
    Then the agent "Echo" is running a turn
    When the user closes the window
    Then Sapphire waits to quit for "Echo", which is "running a turn"
    When the user interrupts and quits
    Then Sapphire quits by itself

  Scenario: What went wrong while stopping stays offered until the user quits
    When the user sends the agent prompt "ignore-interrupt"
    Then the agent "Echo" is running a turn
    When Sapphire is asked to quit
    Then Sapphire waits to quit for "Echo", which is "running a turn"
    When the user interrupts and quits
    Then Sapphire shows that an agent had to be terminated
    When the user dismisses the pause progress
    Then the toolbar shows "Stopped"
    When the user opens the pause status
    And the user quits from the shutdown problems
    Then Sapphire quits by itself
