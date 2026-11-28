@claude-command
Feature: The Claude Code command
  A profile's agents run Claude Code through the command its Claude Launch Settings name, `claude` unless set.
  Each agent keeps the command it was created with, and looks it up again at every launch.

  Background:
    Given an origin-backed profile "Work" with repo "api"
    And Devora is running with no sessions open
    And the Agent Hub is open

  Scenario: A new agent runs the profile's command, and an agent made before it keeps its own
    Given an agent "Echo" was created
    And the command "claude-gateway" is installed
    And the profile "Work" runs Claude Code through the command "claude-gateway"
    When the user closes the agent "Echo" and opens it again
    Then the agent "Echo" should be idle
    And the agent "Echo" should run claude itself
    Given the Agent Hub is open
    And an agent "Foxtrot" was created
    When the user opens the chat for "Foxtrot"
    Then the agent "Foxtrot" should be idle
    And the agent "Foxtrot" should run through the command

  Scenario: A launch that fails says why in the chat, and Retry launches the agent once it's fixed
    Given the profile "Work" runs Claude Code through the command "claude-gateway"
    And an agent "Echo" was created
    Then the agent chat should say its launch failed: "isn't a file you can run."
    And no error banners should be visible
    When the command "claude-gateway" is installed, but is not Claude Code
    And the user retries the agent's launch
    Then the agent chat should say its launch failed: "didn't answer --version as Claude Code does"
    When the command "claude-gateway" is installed
    And the user retries the agent's launch
    Then the agent chat should show no failure
    And the agent "Echo" should be idle
    And the agent "Echo" should run through the command
