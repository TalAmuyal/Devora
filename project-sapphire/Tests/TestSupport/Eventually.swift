import Testing

/// Waits until `condition` holds, for work that has no event to await (a store fold, the core's answer, a view's
/// correction), and records an issue at the caller if it never does.
///
/// The budget is a number of looks, 10 ms apart while the main actor is free, never a time: the tests share the one
/// main actor, so on a busy machine the looks just come later, and the work they wait for gets its turns in between.
/// A deadline in time would expire while other suites hold the main actor, with the work still queued behind them.
/// What catches a real hang is the budget, and the suite's `.timeLimit` when the looks themselves can't run.
@MainActor
public func eventually(
	looks: Int = 500, sourceLocation: SourceLocation = #_sourceLocation, _ condition: () -> Bool
) async throws {
	for _ in 0..<looks {
		if condition() { return }
		try await Task.sleep(for: .milliseconds(10))
	}
	if !condition() { Issue.record("the condition never held, in \(looks) looks", sourceLocation: sourceLocation) }
}

/// `eventually` for a condition that is itself asynchronous (a web view's script, the core's answer), with the same
/// budget of looks.
@MainActor
public func eventually(
	looks: Int = 500, sourceLocation: SourceLocation = #_sourceLocation, _ condition: () async -> Bool
) async throws {
	for _ in 0..<looks {
		if await condition() { return }
		try await Task.sleep(for: .milliseconds(10))
	}
	if !(await condition()) { Issue.record("the condition never held, in \(looks) looks", sourceLocation: sourceLocation) }
}
