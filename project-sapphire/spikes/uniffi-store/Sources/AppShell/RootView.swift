import Store
import SwiftUI

/// The spike's only view entry point: it owns the app store for the window.
public struct AppRoot: View {
	@State private var store = Store()

	public init() {}

	public var body: some View { RootView(store: store) }
}

/// Reads the store and dispatches commands; it never touches the core.
struct RootView: View {
	let store: Store
	@State private var frames = FrameStats()
	@State private var settings = StreamSettings()
	@State private var workspacePath = FileManager.default.currentDirectoryPath
	@State private var lastFrames: FrameSummary?

	var body: some View {
		VStack(alignment: .leading, spacing: 12) {
			HStack {
				TextField("Workspace path", text: $workspacePath)
				Button("Open") { store.dispatch(.openWorkspace(path: workspacePath)) }
			}
			if let ws = store.workspace {
				Text("Workspace \(ws.name): \(ws.entryCount) entries at \(ws.path)")
			}
			if let error = store.lastError {
				Text(error).foregroundStyle(.red)
			}

			HStack {
				Picker("Delivery", selection: $settings.style) {
					ForEach(DeliveryStyle.allCases, id: \.self) { Text($0.rawValue) }
				}
				Picker("UI batching", selection: $settings.batching) {
					ForEach(UIBatching.allCases, id: \.self) { Text($0.rawValue) }
				}
				Button("Stream 10k/s") {
					frames.start()
					store.dispatch(.startStream(settings))
				}
				.disabled(store.isStreaming)
				Button("Clear") { store.dispatch(.clearTranscript) }
			}

			HStack(spacing: 16) {
				Spinner()
				DeltaCounter(store: store)
				if let run = store.lastRun {
					Text(String(
						format: "latency p50 %.2f / p95 %.2f / p99 %.2f ms · CPU %.0f%% · folds %llu",
						run.latencyMs.p50, run.latencyMs.p95, run.latencyMs.p99, run.processCPUPercent, run.mainActorFolds))
				}
				if let f = lastFrames {
					Text(String(format: "%.0f fps · dropped %.1f%% · p99 frame %.1f ms", f.averageFPS, f.droppedPercent, f.intervalMs.p99))
				}
			}
			.font(.callout)

			TranscriptView(store: store)
		}
		.padding()
		.frame(minWidth: 900, minHeight: 600)
		.background(FrameDriver(stats: frames) { store.frameTick() })
		.onChange(of: store.isStreaming) { _, streaming in
			if !streaming { lastFrames = frames.stop() }
		}
		.task {
			if let output = UIBench.outputPath {
				await UIBench.run(store: store, frames: frames, output: output)
			}
		}
	}
}

// Streaming state is read only by these leaf views, so a delta re-evaluates them and not the whole window.

private struct DeltaCounter: View {
	let store: Store

	var body: some View {
		Text("deltas: \(store.deltaCount)").monospacedDigit()
	}
}

private struct TranscriptView: View {
	let store: Store

	var body: some View {
		ScrollView {
			Text(store.transcript.suffix(3_000))
				.font(.body.monospaced())
				.frame(maxWidth: .infinity, alignment: .leading)
		}
		.defaultScrollAnchor(.bottom)
	}
}

/// Animates every frame, so the window renders continuously and jank is visible.
private struct Spinner: View {
	var body: some View {
		TimelineView(.animation) { context in
			let angle = context.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 1) * 360
			Rectangle()
				.fill(.tint)
				.frame(width: 16, height: 16)
				.rotationEffect(.degrees(angle))
		}
	}
}
