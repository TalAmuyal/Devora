import AppKit
import CoreBindings
import EditorPane
import Observation
import SwiftUI

/// Launch arguments, always as `--flag value` pairs (a bare argument makes AppKit open it as a file, and then SwiftUI
/// opens no window):
/// - `--config user|clean`: seed from `~/.config/nvim` (default) or run Neovim with `--clean`
/// - `--ext-messages on|off`
/// - `--latency-bench <out.json>`: type scripted keys, write the timings, quit
/// - `--type <keys>` then `--snapshot <out.png>`: type keys (Neovim notation), render the window to a PNG, quit
enum LaunchOptions {
	static func value(_ flag: String) -> String? {
		let args = CommandLine.arguments
		guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
		return args[i + 1]
	}

	static var useUserConfig: Bool { value("--config") != "clean" }
	static var extMessages: Bool { value("--ext-messages") == "on" }
	static var latencyBenchOutput: String? { value("--latency-bench") }
}

enum Pane: Hashable {
	case left, editor, right
}

@MainActor
@Observable
final class SpikeApp {
	var focus: Pane = .editor
	var status = "starting…"
	private(set) var session: EditorSession?
	@ObservationIgnored let latency = LatencyRecorder()

	func start() async {
		do {
			let home = FileManager.default.homeDirectoryForCurrentUser
			let support = try FileManager.default.url(
				for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true
			).appending(path: "DevoraSapphireSpike")
			let init_ = try writeNvimInit(
				dir: support.appending(path: "nvim").path, userConfigDir: home.appending(path: ".config/nvim").path)
			status = "init \(init_.path) sources \(init_.userInit)"
			// A spike convenience: the host plugin is read from the source tree rather than the app bundle.
			let hostPlugin = URL(filePath: #filePath).deletingLastPathComponent()
				.appending(path: "../../host/devora_host.lua").standardizedFileURL.path
			let path = (ProcessInfo.processInfo.environment["PATH"] ?? "") + ":/opt/homebrew/bin:/usr/local/bin"
			var spec = try nvimSpawnSpec(init: init_.path, hostPlugin: hostPlugin, searchPath: path)
			if !LaunchOptions.useUserConfig, let u = spec.args.firstIndex(of: "-u") {
				spec.args.replaceSubrange(u...(u + 1), with: ["--clean"])
			}

			let drafts = try DraftStore(dir: support.appending(path: "drafts").path)
			let session = EditorSession(spec: spec, drafts: drafts, draftID: "prompt")
			session.onEdge = { [weak self] direction in self?.leaveEditor(direction) }
			session.onDraftSaved = { [weak self] draft in
				self?.status = "draft saved: \(draft.lines.count) lines, cursor \(draft.cursorRow):\(draft.cursorCol)"
			}
			self.session = session
			try await session.start(cols: 80, rows: 24, options: UIOptions(messages: LaunchOptions.extMessages))
			if !session.startupPrompts.isEmpty {
				status = "the Neovim config blocked at startup in mode(s) \(session.startupPrompts) (dismissed)"
			}
			try await session.command("vsplit")
			if let output = LaunchOptions.latencyBenchOutput {
				await runLatencyBench(output: output)
			}
			if let output = LaunchOptions.value("--snapshot") {
				try? await Task.sleep(for: .seconds(1))
				if let keys = LaunchOptions.value("--type") {
					latency.inject?(keys)
					try? await Task.sleep(for: .seconds(1))
				}
				snapshot(to: output)
				await session.shutdown()
				NSApplication.shared.terminate(nil)
			}
		} catch {
			status = "error: \(error)"
		}
	}

	func leaveEditor(_ direction: PaneDirection) {
		switch direction {
		case .left: focus = .left
		case .right: focus = .right
		case .up, .down: break
		}
	}

	func enterEditor(from direction: PaneDirection) {
		focus = .editor
		session?.enter(from: direction)
	}

	private func snapshot(to output: String) {
		guard let view = NSApplication.shared.windows.first?.contentView,
			let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds)
		else { return }
		view.cacheDisplay(in: view.bounds, to: rep)
		try? rep.representation(using: .png, properties: [:])?.write(to: URL(filePath: output))
	}

	private func runLatencyBench(output: String) async {
		try? await Task.sleep(for: .seconds(2))
		focus = .editor
		latency.inject?("i")
		try? await Task.sleep(for: .milliseconds(300))
		latency.reset()
		let text = Array("the quick brown fox jumps over the lazy dog ")
		for i in 0..<300 {
			latency.inject?(String(text[i % text.count]))
			try? await Task.sleep(for: .milliseconds(50))
		}
		latency.inject?("<Esc>")
		let result = LatencyReport(
			config: LaunchOptions.useUserConfig ? "user" : "clean", extMessages: LaunchOptions.extMessages,
			samples: latency.samples.count,
			flushMs: Percentiles(latency.samples.map(\.flushMs)),
			drawnMs: Percentiles(latency.samples.map(\.drawnMs)),
			presentedMs: Percentiles(latency.samples.map(\.presentedMs)))
		let encoder = JSONEncoder()
		encoder.outputFormatting = [.sortedKeys]
		try? encoder.encode(result).write(to: URL(filePath: output))
		await session?.shutdown()
		NSApplication.shared.terminate(nil)
	}
}

struct LatencyReport: Codable {
	var config: String
	var extMessages: Bool
	var samples: Int
	var flushMs: Percentiles
	var drawnMs: Percentiles
	var presentedMs: Percentiles
}

struct Percentiles: Codable {
	var p50 = 0.0, p95 = 0.0, p99 = 0.0, max = 0.0

	init(_ values: [Double]) {
		guard !values.isEmpty else { return }
		let s = values.sorted()
		func at(_ q: Double) -> Double { s[Swift.min(s.count - 1, Int(Double(s.count) * q))] }
		p50 = at(0.5); p95 = at(0.95); p99 = at(0.99); max = s.last!
	}
}

public struct AppRoot: View {
	@State private var app = SpikeApp()

	public init() {}

	public var body: some View {
		VStack(spacing: 0) {
			HStack(spacing: 0) {
				DummyPane(name: "Left pane", towardEditor: "l", isFocused: app.focus == .left) { app.enterEditor(from: .left) }
					.frame(width: 220)
				Divider()
				if let session = app.session {
					EditorPaneView(session: session, latency: app.latency, isFocused: app.focus == .editor) {
						app.focus = .editor
					}
				} else {
					Color.clear
				}
				Divider()
				DummyPane(name: "Right pane", towardEditor: "h", isFocused: app.focus == .right) { app.enterEditor(from: .right) }
					.frame(width: 220)
			}
			Divider()
			Text(app.status).font(.caption).padding(4).frame(maxWidth: .infinity, alignment: .leading)
		}
		.frame(minWidth: 1100, minHeight: 600)
		.task { await app.start() }
	}
}

/// A stand-in for a native Sapphire pane: `Ctrl-<dir>` or `Ctrl-w <dir>` toward the editor moves focus back into Neovim.
private struct DummyPane: View {
	let name: String
	let towardEditor: Character
	let isFocused: Bool
	let enterEditor: () -> Void
	@FocusState private var focused: Bool
	@State private var pendingCtrlW = false

	var body: some View {
		VStack {
			Text(name).font(.title3)
			Text(isFocused ? "focused: Ctrl-\(String(towardEditor)) to go back" : "").font(.caption).foregroundStyle(.secondary)
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity)
		.background(isFocused ? Color.accentColor.opacity(0.15) : Color.clear)
		.focusable()
		.focusEffectDisabled()
		.focused($focused)
		.onKeyPress(phases: .down) { press in
			let ctrl = press.modifiers.contains(.control)
			if ctrl, press.characters == "\u{17}" || press.key.character == "w" {
				pendingCtrlW = true
				return .handled
			}
			let key = press.key.character
			defer { pendingCtrlW = false }
			if ctrl || pendingCtrlW, key == towardEditor {
				enterEditor()
				return .handled
			}
			return .ignored
		}
		.onChange(of: isFocused, initial: true) { _, now in focused = now }
	}
}
