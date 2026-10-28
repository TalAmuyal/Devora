import CoreBindings
import Foundation
import Store
import Testing

@MainActor
struct StoreTests {
	@Test(arguments: DeliveryStyle.allCases, UIBatching.allCases)
	func everyDeliveryPathFoldsAllDeltasInOrder(style: DeliveryStyle, batching: UIBatching) async throws {
		let store = Store()
		store.dispatch(.startStream(StreamSettings(
			ratePerSecond: 0, count: 5_000, deltaLength: 4, style: style, batching: batching)))

		let deadline = ContinuousClock.now + .seconds(10)
		while store.isStreaming, ContinuousClock.now < deadline {
			store.frameTick()
			try await Task.sleep(for: .milliseconds(2))
		}

		let run = try #require(store.lastRun)
		#expect(run.deltas == 5_000)
		#expect(run.outOfOrder == 0)
		#expect(store.deltaCount == 5_000)
		#expect(store.transcript.utf8.count == 20_000)
	}

	@Test func openWorkspaceErrorIsSurfacedInState() async throws {
		let store = Store()
		store.dispatch(.openWorkspace(path: "/nonexistent/workspace"))
		let deadline = ContinuousClock.now + .seconds(2)
		while store.lastError == nil, ContinuousClock.now < deadline {
			try await Task.sleep(for: .milliseconds(5))
		}
		#expect(store.lastError?.contains("NotFound") == true)
		#expect(store.workspace == nil)
	}
}
