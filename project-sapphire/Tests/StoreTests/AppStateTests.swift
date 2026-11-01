import CoreBindings
import Foundation
import Store
import Testing

/// The core's tab state, folded from its event stream on the Swift side.
@MainActor
struct AppStateTests {
	@Test func foldingTheEventsReproducesTheCoresState() async throws {
		let fixture = try Fixture()
		let core = try DevoraCore(configPath: "/nonexistent/config.json", statePath: nil, log: try fixture.log())
		let shell = StateCommand.openSession(title: "Shell", kind: .shell, profilePath: nil)
		try core.dispatch(command: shell)
		let subscription = try core.subscribe()
		let creating = StateCommand.openSession(title: "task", kind: .creatingWorkspace, profilePath: "/profiles/work")
		for command in [shell, creating, shell] { try core.dispatch(command: command) }
		let ids = try core.snapshot().window.tabs.map(\.id)
		for command: StateCommand in [
			.moveTab(tab: ids[0], index: 3),
			.setTitle(tab: ids[1], title: "zsh"),
			.workspaceCreated(tab: ids[2], path: "/ws/1"),
			.closeSession(tab: ids[3]),
			.activateTab(tab: ids[1]),
			.setActiveProfile(profilePath: "/profiles/work"),
		] {
			try core.dispatch(command: command)
		}

		var folded = AppState.empty
		var events = Batches(subscription, maxBatch: 3).makeAsyncIterator()
		let expected = try core.snapshot()
		while folded != expected, let batch = try await events.next() {
			for event in batch { folded.apply(event) }
		}

		#expect(folded == expected)
		#expect(folded.window.tabs.map(\.title) == ["zsh", "task", "Shell"])
	}

	@Test func cancellingTheConsumerClosesTheSubscriptionAndEndsTheSequence() async throws {
		let fixture = try Fixture()
		let core = try DevoraCore(configPath: "/nonexistent/config.json", statePath: nil, log: try fixture.log())
		let subscription = try core.subscribe()
		let (received, signal) = AsyncStream<Void>.makeStream()
		let consumer = Task {
			var batches = 0
			for try await _ in Batches(subscription) {
				batches += 1
				signal.yield()
			}
			return batches
		}
		// After the snapshot, the consumer waits in a pull that nothing but `close()` can end.
		var receivedBatches = received.makeAsyncIterator()
		await receivedBatches.next()

		consumer.cancel()

		#expect(try await consumer.value == 1)
		try core.dispatch(command: .openSession(title: "late", kind: .shell, profilePath: nil))
		#expect(try await subscription.nextBatch(max: 10) == [], "closed subscriptions deliver nothing more")
	}

	@Test func theStoreRendersTabsFromTheCore() async throws {
		let fixture = try Fixture()
		let store = Store(configPath: "/nonexistent/config.json", state: .inMemory, log: try fixture.log())
		let workspace = Workspace(id: "ws-1", path: "/profile/workspaces/ws-1", profilePath: "/profile", title: "Fix the build", repos: ["api"], active: true)

		store.dispatch(.openShell)
		store.dispatch(.openWorkspace(workspace))
		try await eventually { store.app.window.tabs.count == 2 }

		let tabs = store.app.window.tabs
		#expect(tabs.map(\.title) == ["Shell", "Fix the build"])
		#expect(tabs.map(\.session.kind) == [.shell, .workspace(path: "/profile/workspaces/ws-1")])
		#expect(store.app.window.activeTab == tabs[1].id)

		store.dispatch(.closeTab(id: tabs[1].id))
		try await eventually { store.app.window.tabs.count == 1 }
		#expect(store.app.window.activeTab == tabs[0].id)
	}
}

/// Polls instead of sleeping a fixed time: events reach the store asynchronously.
///
/// The condition is checked once more after the deadline: the tests share the main actor, so a poll can wake past the deadline with the fold it waits for queued just before it.
@MainActor
func eventually(timeout: Duration = .seconds(5), sourceLocation: SourceLocation = #_sourceLocation, _ condition: () -> Bool) async throws {
	let deadline = ContinuousClock.now + timeout
	while !condition() {
		guard ContinuousClock.now < deadline else {
			Issue.record("condition not met within \(timeout)", sourceLocation: sourceLocation)
			return
		}
		try await Task.sleep(for: .milliseconds(10))
	}
}
