Feature: Workspace sessions
  Opening a workspace starts a session whose terminal runs the profile's default app in the workspace's repo.

  Background:
    Given Devora is running with no sessions open

  Scenario: A workspace session runs the profile's default app in the workspace's repo
    Given a profile "Work" with 1 active workspaces
    And the profile's default app prints the directory it starts in
    When the user opens the workspace "ws-1"
    Then there should be 1 session
    And the terminal should contain "default app in ws-1/test-repo"
