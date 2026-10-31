import CoreBindings

/// A core subscription: a stream pulled in batches (ADR-010).
public protocol BatchSubscription: Sendable {
	associatedtype Item: Sendable
	func nextBatch(max: UInt32) async throws -> [Item]
	func close() throws
}

extension StateSubscription: BatchSubscription {}
extension ErrorSubscription: BatchSubscription {}

/// A core subscription as an `AsyncSequence` of batches.
///
/// It ends when the subscription is closed, and cancelling the consuming task closes the subscription: Swift cancellation doesn't reach the Rust future, so `close()` is what wakes a pending pull (ADR-010).
public struct Batches<Subscription: BatchSubscription>: AsyncSequence, Sendable {
	public typealias Element = [Subscription.Item]

	let subscription: Subscription
	let maxBatch: UInt32

	public init(_ subscription: Subscription, maxBatch: UInt32 = 1024) {
		self.subscription = subscription
		self.maxBatch = maxBatch
	}

	public func makeAsyncIterator() -> Iterator {
		Iterator(subscription: subscription, maxBatch: maxBatch)
	}

	public struct Iterator: AsyncIteratorProtocol {
		let subscription: Subscription
		let maxBatch: UInt32

		public mutating func next() async throws -> Element? {
			let batch = try await withTaskCancellationHandler {
				try await subscription.nextBatch(max: maxBatch)
			} onCancel: { [subscription] in
				try? subscription.close()
			}
			return batch.isEmpty ? nil : batch
		}
	}
}
