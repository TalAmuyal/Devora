import AppKit
import Foundation
import Store

/// Scripted UI measurement, one configuration per process so a run can't inherit another run's backlog:
/// `SapphireSpike --ui-bench <out.jsonl> --run idle|<style>/<batching>` appends one JSON line and quits.
/// Arguments must come in `--flag value` pairs: AppKit treats a bare argument as a file to open, and then SwiftUI opens
/// no window.
@MainActor
enum UIBench {
	static var outputPath: String? { argument(after: "--ui-bench") }
	static var label: String { argument(after: "--run") ?? "idle" }

	private static func argument(after flag: String) -> String? {
		let args = CommandLine.arguments
		guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
		return args[i + 1]
	}

	struct Result: Encodable {
		let label: String
		let run: RunSummary?
		let frames: FrameSummary
	}

	static func run(store: Store, frames: FrameStats, output: String) async {
		try? await Task.sleep(for: .seconds(2))  // let the window settle
		var run: RunSummary?
		frames.start()
		let parts = label.split(separator: "/").map(String.init)
		if parts.count == 2, let style = DeliveryStyle(rawValue: parts[0]), let batching = UIBatching(rawValue: parts[1]) {
			store.dispatch(.startStream(StreamSettings(
				ratePerSecond: 10_000, count: 50_000, style: style, batching: batching)))
			// The stream lasts 5 s; a mode that is still folding 3 s later can't keep up, so record it as such.
			let deadline = ContinuousClock.now + .seconds(8)
			while store.isStreaming, ContinuousClock.now < deadline {
				try? await Task.sleep(for: .milliseconds(20))
			}
			if store.isStreaming { store.dispatch(.cancelStream) }
			run = store.lastRun
		} else {
			try? await Task.sleep(for: .seconds(5))
		}
		let encoder = JSONEncoder()
		encoder.outputFormatting = [.sortedKeys]
		let line = String(decoding: try! encoder.encode(Result(label: label, run: run, frames: frames.stop())), as: UTF8.self)
		let handle = FileHandle(forWritingAtPath: output) ?? {
			FileManager.default.createFile(atPath: output, contents: nil)
			return FileHandle(forWritingAtPath: output)!
		}()
		handle.seekToEndOfFile()
		handle.write(Data((line + "\n").utf8))
		try? handle.close()
		NSApplication.shared.terminate(nil)
	}
}
