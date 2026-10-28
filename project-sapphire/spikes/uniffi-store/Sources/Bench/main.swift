// Headless measurements for #206. Usage: Bench raw | throughput | rate | panic-infallible
import CoreBindings
import Foundation
import Store
import Synchronization

let encoder: JSONEncoder = {
	let e = JSONEncoder()
	e.outputFormatting = [.sortedKeys]
	return e
}()

func emit(_ label: String, _ value: some Encodable) {
	let json = String(decoding: try! encoder.encode(value), as: UTF8.self)
	print("\(label)\t\(json)")
}

/// Runs one stream through the real Store, ticking per-frame batching at 120 Hz like a display link would.
@MainActor
func runStore(_ settings: StreamSettings) async -> RunSummary {
	let store = Store()
	store.dispatch(.startStream(settings))
	while store.isStreaming {
		store.frameTick()
		try? await Task.sleep(for: .microseconds(8_333))
	}
	return store.lastRun!
}

struct RawResult: Encodable {
	let style: String
	let events: UInt64
	let seconds: Double
	let eventsPerSecond: Double
	let nsPerEvent: Double
}

/// Boundary cost only: no main-actor hop, no Store.
final class CountingSink: EventSink {
	let count = Atomic<UInt64>(0)
	let done = Mutex<CheckedContinuation<Void, Never>?>(nil)

	func onEvent(event: CoreEvent) { record([event]) }
	func onEvents(events: [CoreEvent]) { record(events) }

	private func record(_ events: [CoreEvent]) {
		count.add(UInt64(events.count), ordering: .relaxed)
		if case .streamEnded = events.last {
			done.withLock { $0?.resume(); $0 = nil }
		}
	}
}

func raw(_ style: DeliveryStyle, count: UInt64) async -> RawResult {
	let core = SpikeCore()
	let start = DispatchTime.now().uptimeNanoseconds
	switch style {
	case .callback, .callbackBatched:
		let sink = CountingSink()
		core.setSink(sink: sink)
		await withCheckedContinuation { cont in
			sink.done.withLock { $0 = cont }
			_ = try! core.startStream(config: StreamConfig(
				ratePerSec: 0, count: count, deltaLen: 8, delivery: style == .callback ? .callback : .callbackBatched))
		}
	case .pull, .pullBatched:
		_ = try! core.startStream(config: StreamConfig(ratePerSec: 0, count: count, deltaLen: 8, delivery: .pull))
		var finished = false
		while !finished {
			let events = style == .pull ? [await core.nextEvent()!] : await core.nextEvents(max: 0)
			if case .streamEnded = events.last { finished = true }
		}
	}
	let seconds = Double(DispatchTime.now().uptimeNanoseconds - start) / 1e9
	return RawResult(
		style: style.rawValue, events: count, seconds: seconds,
		eventsPerSecond: Double(count) / seconds, nsPerEvent: seconds * 1e9 / Double(count))
}

let mode = CommandLine.arguments.dropFirst().first ?? "rate"
switch mode {
case "raw":
	for style in DeliveryStyle.allCases {
		_ = await raw(style, count: 50_000)  // warm-up
		emit("raw", await raw(style, count: 500_000))
	}
case "throughput":
	for style in DeliveryStyle.allCases {
		for batching in UIBatching.allCases {
			emit("throughput", await runStore(StreamSettings(
				ratePerSecond: 0, count: 200_000, style: style, batching: batching)))
		}
	}
case "rate":
	for style in DeliveryStyle.allCases {
		for batching in UIBatching.allCases {
			emit("rate", await runStore(StreamSettings(
				ratePerSecond: 10_000, count: 50_000, style: style, batching: batching)))
		}
	}
case "panic-infallible":
	print("calling an infallible Rust function that panics…")
	_ = SpikeCore().panicSync()
	print("still alive (unexpected)")
default:
	print("usage: Bench raw | throughput | rate | panic-infallible")
	exit(2)
}
