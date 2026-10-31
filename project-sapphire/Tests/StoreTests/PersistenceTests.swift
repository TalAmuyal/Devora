import CoreBindings
import Foundation
import Store
import Testing

/// The store on an app state kept in a file: what a launch restores, and the problems it can meet opening it.
@MainActor
struct PersistenceTests {
	@Test func aLaunchRestoresTheTabs() async throws {
		let fixture = try Fixture()
		let statePath = fixture.root.appending(path: "state/state.db").path
		var first: Store? = Store(configPath: fixture.configPath, state: .file(path: statePath), log: try fixture.log())
		#expect(first!.hasNoTabs())
		first!.dispatch(.openShell)
		first!.dispatch(.openShell)
		try await eventually { first!.app.window.tabs.count == 2 }
		first!.dispatch(.activateTab(id: first!.app.window.tabs[0].id))
		try await eventually { first!.app.window.activeTab == first!.app.window.tabs[0].id }
		let left = first!.app
		first = nil

		let second = Store(configPath: fixture.configPath, state: .file(path: statePath), log: try fixture.log())

		#expect(!second.hasNoTabs(), "read from the core before any event folds")
		try await eventually { !second.app.window.tabs.isEmpty }
		#expect(second.startupProblem == nil)
		#expect(second.app == left)
	}

	@Test func anUnreadableStateIsSetAsideToStartFresh() async throws {
		let fixture = try Fixture()
		let statePath = fixture.root.appending(path: "state.db").path
		let corrupt = Data(repeating: 0x5a, count: 8192)
		try corrupt.write(to: URL(filePath: statePath))

		let store = Store(configPath: fixture.configPath, state: .file(path: statePath), log: try fixture.log())
		guard case .stateUnreadable(let path, let reason) = store.startupProblem else {
			Issue.record("expected an unreadable state, got \(String(describing: store.startupProblem))")
			return
		}
		#expect(path == statePath)
		#expect(reason.contains("couldn't open the app state"))

		store.dispatch(.startFresh)

		#expect(store.startupProblem == nil)
		#expect(store.hasNoTabs())
		let kept = try FileManager.default.contentsOfDirectory(atPath: fixture.root.path).filter { $0.hasPrefix("state-unreadable-") }
		#expect(kept.count == 1)
		#expect(try Data(contentsOf: fixture.root.appending(path: kept[0])) == corrupt)
	}

	@Test func aStateHeldByAnotherStoreIsInUse() throws {
		let fixture = try Fixture()
		let statePath = fixture.root.appending(path: "state.db").path
		let first = Store(configPath: fixture.configPath, state: .file(path: statePath), log: try fixture.log())

		let second = Store(configPath: fixture.configPath, state: .file(path: statePath), log: try fixture.log())

		#expect(first.startupProblem == nil)
		#expect(second.startupProblem == .stateInUse(path: statePath))
	}
}
