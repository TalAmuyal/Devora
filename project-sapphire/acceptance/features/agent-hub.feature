Feature: Agent Hub
  The Agent Hub lists, creates, opens and deletes the selected profile's Agent Spaces (ADR-012).
  Agents run the core's fake Claude CLI here, never Claude itself.

  Background:
    Given an origin-backed profile "Work" with repo "api"
    And Devora is running with no sessions open

  Scenario: Ctrl+S opens the Agent Hub, and switches between the hubs inside one
    When the user presses Ctrl+S
    Then the Agent Hub should be visible
    When the user presses Ctrl+S
    Then the Workspace Hub should be visible
    And the Agent Hub should not be visible
    When the user presses Ctrl+S
    Then the Agent Hub should be visible
    And the Workspace Hub should not be visible

  Scenario: A new agent is created with the template's fields and opens in its tab
    Given the Agent Hub is open
    When the user opens the New Agent dialog
    Then the New Agent dialog should show the field "agent.role" from "Default"
    When the user names the new agent "Echo"
    Then the New Agent preview should contain "You are Echo and you are working with me as a general-purpose assistant."
    When the user selects the repo "api" for the new agent
    And the user creates the agent
    Then an agent tab "Echo" should be active
    And the agent "Echo" should be idle
    And the agent "Echo" should have a worktree of "api" and a memory directory

  Scenario: A template field only the form fills is required, and can be saved to the profile
    Given the profile's agent template is "{{agent.name}} works on {{team}}"
    And the Agent Hub is open
    When the user opens the New Agent dialog
    Then the New Agent dialog should require the field "team"
    When the user names the new agent "Echo"
    Then the agent can't be created yet
    When the user fills the agent field "team" with "payments"
    And the user saves the agent field "team"
    Then the profile's agent config should have the input "team" set to "payments"
    And the agent can be created

  Scenario: Deleting an open agent asks first, closes its tab, then removes it
    Given the Agent Hub is open
    And an agent "Echo" was created
    When the user deletes the agent "Echo"
    And the user confirms the dialog
    Then the Agent Hub should list no agents
    And there should be 0 sessions
    And the agent space "Echo" should not exist

  Scenario: An open agent is paused on quit and resumed on the next launch
    Given the Agent Hub is open
    And an agent "Echo" was created
    When Sapphire is quit and relaunched
    Then an agent tab "Echo" should be active
    And the agent "Echo" should be idle

  Scenario: Unchecking "Include coding instructions" replaces Claude Code's system prompt
    Given the Agent Hub is open
    When the user opens the New Agent dialog
    And the user names the new agent "Echo"
    And the user unchecks including coding instructions
    And the user creates the agent
    Then the agent "Echo" should be idle
    And Claude should have been launched with the instructions as its whole system prompt

  Scenario: The New Agent dialog starts with opus and leaves the effort to Claude Code when none is set
    Given the Agent Hub is open
    When the user opens the New Agent dialog
    And the user names the new agent "Echo"
    Then the New Agent dialog should start with the model "opus" and Claude Code's default effort
    And the New Agent dialog should say the space is created at "agent-spaces/Echo" in the profile
    When the user creates the agent
    Then the agent "Echo" should be idle
    And Claude should have been launched with the model "opus" and no effort

  Scenario: A chosen model and effort are what Claude is launched with
    Given the Agent Hub is open
    When the user opens the New Agent dialog
    And the user names the new agent "Echo"
    And the user sets the new agent's model to "sonnet" and its effort to "high"
    And the user creates the agent
    Then the agent "Echo" should be idle
    And Claude should have been launched with the model "sonnet" and the effort "high"
