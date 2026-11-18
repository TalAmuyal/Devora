Feature: Hangs the watchdog names

  @blocked
  Scenario: A step blocked on a child process
    When a step waits synchronously for a child process that doesn't end

  @lingering
  Scenario: A run that doesn't exit
    When a step leaves something for the process to wait for

  @passing
  Scenario: A run that ends
    When a step passes
