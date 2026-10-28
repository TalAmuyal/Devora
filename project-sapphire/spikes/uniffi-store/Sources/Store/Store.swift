import CoreBindings
import Darwin
import Foundation
import Observation
import Synchronization

/// How events cross from Rust to Swift (#206 question 1).
public enum DeliveryStyle: String, Sendable, CaseIterable, Codable {
	/// (a) foreign-implemented `EventSink`, one call per event
	case callback
	/// (a) foreign-implemented `EventSink`, one call per ~1 ms producer tick
	case callbackBatched
	/// (b) `await core.nextEvent()` in a loop
	case pull
	/// (b) `await core.nextEvents(max:)` in a loop
	case pullBatched
}

/// How events reach the main actor (#206 question 2).
public enum UIBatching: String, Sendable, CaseIterable, Codable {
	/// every delivery hops to the main actor and is folded immediately
	case perDelivery
	/// deliveries are buffered off-main and folded once per display frame (`frameTick`)
	case perFrame
}

public struct StreamSettings: Sendable, Codable, Equatable {
	public var ratePerSecond: UInt32
	public var count: UInt64
	public var deltaLength: UInt32
	public var style: DeliveryStyle
	public var batching: UIBatching

	public init(
		ratePerSecond: UInt32 = 10_000, count: UInt64 = 50_000, deltaLength: UInt32 = 8,
		style: DeliveryStyle = .callback, batching: UIBatching = .perDelivery
	) {
		self.ratePerSecond = ratePerSecond
		self.count = count
		self.deltaLength = deltaLength
		self.style = style
		self.batching = batching
	}
}

public enum Command: Sendable {
	case openWorkspace(path: String)
	case startStream(StreamSettings)
	/// Ends the current run where it is; events still in flight from it are ignored.
	case cancelStream
	case clearTranscript
}

public struct RunSummary: Sendable, Codable {
	public var settings: StreamSettings
	/// False when the run was cancelled before every delta was folded.
	public var completed: Bool
	public var deltas: UInt64
	public var wallSeconds: Double
	public var eventsPerSecond: Double
	/// Mutations of observed state; per-frame batching folds many deltas into one.
	public var mainActorFolds: UInt64
	/// Deltas whose sequence number wasn't the previous one plus one.
	public var outOfOrder: UInt64
	/// Produced in Rust -> folded on the main actor.
	public var latencyMs: Percentiles
	/// Whole-process CPU (Rust producer + Swift) as a percentage of one core.
	public var processCPUPercent: Double
}

public struct Percentiles: Sendable, Codable {
	public var p50: Double
	public var p95: Double
	public var p99: Double
	public var max: Double

	public init(_ samples: [Double]) {
		guard !samples.isEmpty else {
			self.init(p50: 0, p95: 0, p99: 0, max: 0)
			return
		}
		let sorted = samples.sorted()
		func at(_ q: Double) -> Double { sorted[min(sorted.count - 1, Int(Double(sorted.count) * q))] }
		self.init(p50: at(0.50), p95: at(0.95), p99: at(0.99), max: sorted.last!)
	}

	public init(p50: Double, p95: Double, p99: Double, max: Double) {
		self.p50 = p50
		self.p95 = p95
		self.p99 = p99
		self.max = max
	}
}

/// The single app store: views dispatch `Command`s, and core events are folded into observable state on the main actor.
@MainActor
@Observable
public final class Store {
	public private(set) var workspace: WorkspaceInfo?
	public private(set) var lastError: String?
	public private(set) var transcript = ""
	public private(set) var deltaCount: UInt64 = 0
	public private(set) var isStreaming = false
	public private(set) var lastRun: RunSummary?

	@ObservationIgnored private let core: SpikeCore
	@ObservationIgnored private var run: RunState?
	@ObservationIgnored private var nextRunID = 0

	public init(core: SpikeCore = SpikeCore()) {
		self.core = core
	}

	public func dispatch(_ command: Command) {
		switch command {
		case .openWorkspace(let path):
			Task {
				do {
					workspace = try await core.openWorkspace(path: path)
					lastError = nil
				} catch {
					lastError = String(describing: error)
				}
			}
		case .startStream(let settings):
			startStream(settings)
		case .cancelStream:
			finishRun(completed: false)
		case .clearTranscript:
			transcript = ""
			deltaCount = 0
		}
	}

	/// Folds everything buffered since the last frame. Call once per display frame when batching per frame.
	public func frameTick() {
		guard let run else { return }
		let events = run.inbox.drain()
		if !events.isEmpty { fold(events, runID: run.id) }
	}

	private func startStream(_ settings: StreamSettings) {
		guard !isStreaming else { return }
		isStreaming = true
		lastRun = nil
		nextRunID += 1
		let run = RunState(id: nextRunID, settings: settings)
		self.run = run

		// One core object per stream, like one AgentSession per conversation: a cancelled run's leftovers can't mix
		// into the next one.
		let session = SpikeCore()
		let route = makeRoute(settings.batching, run: run)
		let delivery: Delivery
		switch settings.style {
		case .callback, .callbackBatched:
			session.setSink(sink: StoreSink(route: route))
			delivery = settings.style == .callback ? .callback : .callbackBatched
		case .pull, .pullBatched:
			delivery = .pull
			Task.detached {
				await Self.pullLoop(core: session, batched: settings.style == .pullBatched, route: route)
			}
		}
		do {
			_ = try session.startStream(config: StreamConfig(
				ratePerSec: settings.ratePerSecond, count: settings.count,
				deltaLen: settings.deltaLength, delivery: delivery))
		} catch {
			lastError = String(describing: error)
			isStreaming = false
		}
	}

	private nonisolated static func pullLoop(core: SpikeCore, batched: Bool, route: Route) async {
		while true {
			let events: [CoreEvent]
			if batched {
				events = await core.nextEvents(max: 0)
			} else {
				guard let event = await core.nextEvent() else { return }
				events = [event]
			}
			await route(events)
			if events.contains(where: { if case .streamEnded = $0 { true } else { false } }) { return }
		}
	}

	private typealias Route = @Sendable ([CoreEvent]) async -> Void

	private func makeRoute(_ batching: UIBatching, run: RunState) -> Route {
		switch batching {
		case .perDelivery:
			return { [weak self, id = run.id] events in
				await self?.fold(events, runID: id)
			}
		case .perFrame:
			return { [inbox = run.inbox] events in inbox.append(events) }
		}
	}

	private func fold(_ events: [CoreEvent], runID: Int) {
		guard run?.id == runID else { return }
		let now = DispatchTime.now().uptimeNanoseconds
		run?.folds += 1
		var appended = ""
		var deltas: UInt64 = 0
		var ended = false
		for event in events {
			switch event {
			case .delta(let seq, let text, let producedNs):
				if let run, seq != run.nextSeq { self.run?.outOfOrder += 1 }
				run?.nextSeq = seq + 1
				appended += text
				deltas += 1
				run?.latencies.append(Double(now &- producedNs) / 1e6)
			case .streamEnded:
				ended = true
			}
		}
		transcript += appended
		deltaCount += deltas
		if ended { finishRun(completed: true) }
	}

	private func finishRun(completed: Bool) {
		guard let run else { return }
		let wall = Double(DispatchTime.now().uptimeNanoseconds - run.startNs) / 1e9
		let cpu = cpuSeconds() - run.startCPU
		lastRun = RunSummary(
			settings: run.settings,
			completed: completed,
			deltas: UInt64(run.latencies.count),
			wallSeconds: wall,
			eventsPerSecond: Double(run.latencies.count) / wall,
			mainActorFolds: run.folds,
			outOfOrder: run.outOfOrder,
			latencyMs: Percentiles(run.latencies),
			processCPUPercent: cpu / wall * 100)
		self.run = nil
		isStreaming = false
	}
}

private struct RunState {
	let id: Int
	let settings: StreamSettings
	let inbox = Inbox()
	let startNs = DispatchTime.now().uptimeNanoseconds
	let startCPU = cpuSeconds()
	var folds: UInt64 = 0
	var nextSeq: UInt64 = 0
	var outOfOrder: UInt64 = 0
	var latencies: [Double] = []

	init(id: Int, settings: StreamSettings) {
		self.id = id
		self.settings = settings
		latencies.reserveCapacity(Int(settings.count))
	}
}

/// Off-main buffer for per-frame batching.
private final class Inbox: Sendable {
	private let events = Mutex<[CoreEvent]>([])

	func append(_ new: [CoreEvent]) {
		events.withLock { $0.append(contentsOf: new) }
	}

	func drain() -> [CoreEvent] {
		events.withLock { pending in
			defer { pending.removeAll(keepingCapacity: true) }
			return pending
		}
	}
}

/// (a) The foreign-implemented sink. Rust calls it on its producer thread.
private final class StoreSink: EventSink {
	private let route: @Sendable ([CoreEvent]) async -> Void
	private let queue = SerialHop()

	init(route: @escaping @Sendable ([CoreEvent]) async -> Void) {
		self.route = route
	}

	func onEvent(event: CoreEvent) { queue.submit([event], route) }
	func onEvents(events: [CoreEvent]) { queue.submit(events, route) }
}

/// Hands deliveries from the Rust thread to the route in order, one at a time.
private final class SerialHop: Sendable {
	private let continuation: AsyncStream<@Sendable () async -> Void>.Continuation

	init() {
		let (stream, continuation) = AsyncStream<@Sendable () async -> Void>.makeStream()
		self.continuation = continuation
		Task.detached {
			for await job in stream { await job() }
		}
	}

	deinit { continuation.finish() }

	func submit(_ events: [CoreEvent], _ route: @escaping @Sendable ([CoreEvent]) async -> Void) {
		continuation.yield { await route(events) }
	}
}

func cpuSeconds() -> Double {
	var usage = rusage()
	getrusage(RUSAGE_SELF, &usage)
	func secs(_ t: timeval) -> Double { Double(t.tv_sec) + Double(t.tv_usec) / 1e6 }
	return secs(usage.ru_utime) + secs(usage.ru_stime)
}
