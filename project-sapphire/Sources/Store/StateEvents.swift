import CoreBindings

/// A core state subscription as an `AsyncSequence` of event batches.
///
/// It ends when the subscription is closed, and cancelling the consuming task closes the subscription: Swift cancellation doesn't reach the Rust future, so `close()` is what wakes a pending pull (ADR-010).
public struct StateEvents: AsyncSequence, Sendable {
	public typealias Element = [StateEvent]

	let subscription: StateSubscription
	let maxBatch: UInt32

	public init(_ subscription: StateSubscription, maxBatch: UInt32 = 1024) {
		self.subscription = subscription
		self.maxBatch = maxBatch
	}

	public func makeAsyncIterator() -> Iterator {
		Iterator(subscription: subscription, maxBatch: maxBatch)
	}

	public struct Iterator: AsyncIteratorProtocol {
		let subscription: StateSubscription
		let maxBatch: UInt32

		public mutating func next() async throws -> [StateEvent]? {
			let batch = try await withTaskCancellationHandler {
				try await subscription.nextBatch(max: maxBatch)
			} onCancel: { [subscription] in
				try? subscription.close()
			}
			return batch.isEmpty ? nil : batch
		}
	}
}
