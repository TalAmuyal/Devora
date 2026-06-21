@clone-repo
Feature: Clone a repo into a profile
  From the Command Palette or the Workspace Hub's New Task form, the user can paste a git URL and clone the repo into the active profile's repos/ directory, checked out detached.
  Progress streams in a dialog and the cloned repo is auto-discovered into the profile's repo list.

  Background:
    Given Ember is running

  Scenario: Clone a repo into the active profile from the Command Palette
    Given an origin-backed profile "Work" with repo "test-repo"
    And a bare repo "cloned-repo" available to clone
    And the Workspace Hub is open
    When the user runs the "Clone Repo into Profile" palette command
    And the user clones the repo "cloned-repo"
    Then the repo "cloned-repo" should exist detached in profile "Work"

  Scenario: Clone a repo from the Workspace Hub New Task form
    Given an origin-backed profile "Work" with repo "test-repo"
    And a bare repo "hub-repo" available to clone
    And the Workspace Hub is open
    When the user opens the New Task form
    And the user clicks "Clone Repo" in the New Task form
    And the user clones the repo "hub-repo"
    Then the repo "hub-repo" should exist detached in profile "Work"
    And the New Task form repo list should include "hub-repo"
