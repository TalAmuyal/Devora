Feature: Agent chat panel
  The agent's typed transcript and unsent prompt belong to its persistent space.

  Background:
    Given an origin-backed profile "Work" with repo "api"
    And Devora is running with no sessions open
    And the Agent Hub is open
    And an agent "Echo" was created
    When the user opens the chat for "Echo"

  Scenario: Sending a prompt renders typed rows and sanitizes content
    When the user sends the agent prompt "Hello <img src=x onerror=alert(1)><script>alert(2)</script>"
    Then the agent transcript contains "Hello"
    And the agent transcript has a "chat-user" item
    And the agent transcript has a "chat-assistant" item
    And the agent transcript has no executable content
    And the agent prompt draft is ""

  Scenario: Unsent prompts survive closing and reopening the agent
    When the user drafts the agent prompt "Keep this unsent"
    And the user closes and reopens the agent chat "Echo"
    Then the agent prompt draft is "Keep this unsent"

  Scenario: Permission cards can be answered from the tab and disappear
    When the user sends the agent prompt "permission"
    Then the agent chat shows a permission card
    And the agent transcript contains "localSettings"
    When the user allows the agent permission with Command+Y
    Then the agent chat has no permission card
    And the agent "Echo" should be idle

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

  Scenario: Scrolling loads older history without moving the visible row and restores it on relaunch
    Given the agent chat has 90 completed test turns
    When the user scrolls up for older agent history
    Then earlier agent history loads without moving the visible row
    When the user scrolls to the first agent history item
    And Sapphire is quit and relaunched
    Then the first agent history item is restored at the top

  @native-transcript
  Scenario: The native transcript loads older history without moving the visible row and restores it on relaunch
    Given the agent chat has 90 completed test turns
    When the user scrolls up for older agent history
    Then earlier agent history loads without moving the visible row
    When the user scrolls to the first agent history item
    And Sapphire is quit and relaunched
    Then the first agent history item is restored at the top

  Scenario: Thinking and tool details start collapsed and the Judgess badge is shown
    When the user sends the agent prompt "chat-fixture"
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
    When the user sends the agent prompt "stream"
    And the user presses Escape and Control+C in the agent prompt
    Then the agent turn is still running
    When the user stops the agent turn
    Then the agent "Echo" should be idle

  Scenario: The context bar uses the actor's usage and profile threshold
    When the user sends the agent prompt "Context metrics"
    Then the agent context bar has a known window and threshold 80
