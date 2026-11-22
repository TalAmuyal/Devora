Feature: Agent chat panel
  The agent's typed transcript and unsent prompt belong to its persistent space.

  Background:
    Given an origin-backed profile "Work" with repo "api"
    And Devora is running with no sessions open
    And the Agent Hub is open
    And an agent "Echo" was created
    When the user opens the chat for "Echo"

  Scenario: Sending a prompt renders typed rows and shows markup as text
    When the user sends the agent prompt "Hello <img src=x onerror=alert(1)><script>alert(2)</script>"
    Then the agent transcript contains "Hello"
    And the agent transcript has a "chat-user" item
    And the agent transcript has a "chat-assistant" item
    And the agent transcript shows "<img src=x onerror=alert(1)><script>alert(2)</script>" as text
    And the agent prompt draft is ""

  Scenario: A Mermaid diagram in a reply is drawn, and one that doesn't parse shows its source
    When the user sends the agent prompt "mermaid"
    Then the agent transcript draws its Mermaid diagram
    When the user sends the agent prompt "bad-mermaid"
    Then the agent transcript shows the Mermaid source "A[Client --> (((" with a warning

  Scenario: Unsent prompts survive closing and reopening the agent
    When the user drafts the agent prompt "Keep this unsent"
    And the user closes and reopens the agent chat "Echo"
    Then the agent prompt draft is "Keep this unsent"

  Scenario: Permission cards can be answered from the tab and disappear
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    And the agent chat's turn status is "Waiting for your answer: Bash"
    And the agent transcript contains "localSettings"
    When the user allows the agent permission with Command+Y
    Then the agent chat has no permission card
    And the agent "Echo" should be idle
    And the agent chat shows no turn status
    And the agent transcript has no "Turn completed" row

  Scenario: A withdrawn question disappears
    When the user sends the agent prompt "cancel-permission"
    Then the agent chat has no permission card
    And the agent "Echo" should be idle

  Scenario: Pause waits for a permission-held turn to finish
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    When the user requests Pause in the agent chat
    Then the agent "Echo" should be pausing
    When the user allows the agent permission with Command+Y
    Then the agent chat is paused
    When the user resumes the agent chat
    Then the agent "Echo" should be idle

  Scenario: Pausing stops the agent's background work, and resuming tells it what was stopped
    When the user shows the agent's system messages
    And the user sends the agent prompt "background-work"
    Then the agent "Echo" should be idle
    When the user requests Pause in the agent chat
    Then the agent chat is paused
    When the user resumes the agent chat
    Then the agent transcript contains "Devora paused the agent and stopped a watcher (shell), a CI wait (subagent)"
    And the agent "Echo" should be idle

  Scenario: Scrolling loads older history without moving the visible row and restores it on relaunch
    Given the agent chat has 90 completed test turns
    When the user scrolls up for older agent history
    Then earlier agent history loads without moving the visible row
    When the user scrolls to the first agent history item
    And Sapphire is quit and relaunched
    Then the first agent history item is restored at the top

  Scenario: Thinking and tool details start collapsed and the Judgess badge is shown
    When the user shows the agent's system messages
    And the user sends the agent prompt "chat-fixture"
    Then the agent transcript has a "chat-thinking" item
    And the agent transcript has a "chat-tool" item
    And the agent tool has a Judgess badge
    And the agent thinking and tool details are collapsed
    When the user expands the agent thinking and tool details
    Then the expanded agent details show their content

  Scenario: The profile's Enter-sends setting applies when returning to the editor
    Given the Workspace Hub is open
    When the user presses "P"
    Then the Settings Hub should be visible
    When the user opens the "Work" settings detail
    And the user switches "agent.enter-sends" to "On"
    And the user returns to the agent prompt
    And the user drafts the agent prompt "Send with Enter"
    And the user sends the agent prompt with plain Enter
    Then the agent prompt draft is ""
    And the agent transcript contains "Send with Enter"

  Scenario Outline: Permission shortcuts deny or always allow
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    When the user answers the agent permission with "<shortcut>"
    Then the agent chat has no permission card
    And the agent "Echo" should be idle

    Examples:
      | shortcut         |
      | Command+N        |
      | Command+Shift+Y  |

  Scenario: Stop interrupts while Escape and Control+C keep the turn running
    When the user shows the agent's system messages
    And the user sends the agent prompt "stream"
    And the user presses Escape and Control+C in the agent prompt
    Then the agent turn is still running
    When the user stops the agent turn
    Then the agent "Echo" should be idle
    And the agent transcript contains "Turn stopped"
    And the agent transcript has no "Turn ended with an error" row

  Scenario: The context bar uses the actor's usage and profile threshold
    When the user sends the agent prompt "Context metrics"
    Then the agent context bar has a known window and threshold 80

  Scenario: The context popover sets this agent's threshold and compacts on demand
    When the user shows the agent's system messages
    And the user sends the agent prompt "Context metrics"
    And the user sets the agent compaction threshold to 50
    Then the agent compacts at 50 for this agent, over the profile threshold 80
    When the user compacts the agent now
    Then the agent transcript contains "Compaction requested"
    And the agent compaction has finished
    When the user resets the agent compaction threshold
    Then the agent context bar has a known window and threshold 80
    When the user requests Pause in the agent chat
    Then the agent chat is paused
    And the context popover refuses Compact Now with "This agent is paused"

  Scenario: The model widget shows the model the replies come from, and the transcript says once where it changed
    When the user sends the agent prompt "model:claude-opus-5-5"
    Then the agent model widget shows "Opus 5.5", as the model it is set to
    When the user sends the agent prompt "model:claude-sonnet-5-5"
    Then the agent model widget shows "Sonnet 5.5", not the model it is set to
    When the user sends the agent prompt "model:claude-sonnet-5-5"
    Then the agent transcript says once "Model changed: Opus 5.5 → Sonnet 5.5"

  Scenario: System messages are hidden until the user shows them, and the choice is the agent's, across a relaunch
    When the user sends the agent prompt "chat-fixture"
    Then the agent "Echo" should be idle
    And the agent transcript contains "A visible answer"
    And the agent transcript does not contain "Bash"
    When the user shows the agent's system messages
    Then the agent transcript has a "chat-tool" item
    And the agent transcript has a "chat-thinking" item
    When Sapphire is quit and relaunched
    Then the agent transcript has a "chat-tool" item
    When the user hides the agent's system messages
    Then the agent transcript does not contain "Bash"
    And the agent transcript contains "A visible answer"

