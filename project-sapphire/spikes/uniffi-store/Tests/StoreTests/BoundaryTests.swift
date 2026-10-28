import CoreBindings
import Foundation
import Testing

/// #206 question 3: errors, panics and cancellation across the UniFFI boundary.
struct BoundaryTests {
	let core = SpikeCore()

	@Test func rustErrBecomesTypedSwiftThrow() async {
		do {
			_ = try await core.openWorkspace(path: "/nonexistent/workspace")
			Issue.record("expected a throw")
		} catch CoreError.NotFound(let path) {
			#expect(path == "/nonexistent/workspace")
		} catch {
			Issue.record("unexpected error: \(error)")
		}
	}

	@Test func panicInFallibleFunctionsIsThrownNotFatal() async {
		#expect { try core.panicSyncFallible() } throws: { "\($0)".contains("deliberate sync panic") }
		await #expect { try await core.panicAsyncFallible() } throws: { "\($0)".contains("deliberate async panic") }
	}

	@Test func swiftTaskCancellationDoesNotReachTheRustFuture() async throws {
		let core = core
		let started = ContinuousClock.now
		let task = Task { await core.sleep(millis: 300) }
		try await Task.sleep(for: .milliseconds(30))
		task.cancel()

		#expect(await task.value == 300)
		#expect(ContinuousClock.now - started >= .milliseconds(290))
		#expect(core.cancellationStats() == CancellationStats(started: 1, completed: 1, droppedBeforeCompletion: 0))
	}

	@Test func cancelTokenBridgesTaskCancellation() async throws {
		let core = core
		let started = ContinuousClock.now
		let task = Task {
			let token = CancelToken()
			return try await withTaskCancellationHandler {
				try await core.sleepCancellable(millis: 2_000, token: token)
			} onCancel: {
				token.cancel()
			}
		}
		try await Task.sleep(for: .milliseconds(30))
		task.cancel()

		await #expect(throws: CoreError.Cancelled) { try await task.value }
		#expect(ContinuousClock.now - started < .milliseconds(200))
	}
}
