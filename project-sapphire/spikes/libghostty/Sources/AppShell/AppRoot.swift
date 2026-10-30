import AppKit
import Metal
import Observation
import ScreenCaptureKit
import SwiftUI
import TerminalKit

/// Launch arguments come in `--flag value` pairs (a bare argument makes AppKit open it as a file, and SwiftUI then opens no window).
enum LaunchOptions {
	static func value(_ flag: String) -> String? {
		let args = CommandLine.arguments
		guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
		return args[i + 1]
	}
}

/// Window layout and focus: a native pane at index -1, terminals from 0.
@MainActor
@Observable
final class SpikeStore {
	var sessions: [TerminalSession] = []
	var focus = 0
	var status = ""
	/// Host chords the router consumed, for the self-test.
	var routedChords: [String] = []
	@ObservationIgnored private var pendingCtrlW = false

	init() {
		if LaunchOptions.value("--bench") == nil {
			sessions = [
				TerminalSession(command: nil, workingDirectory: NSHomeDirectory()),
				TerminalSession(command: "nvim", workingDirectory: NSHomeDirectory()),
				TerminalSession(command: Self.cccCommand(), workingDirectory: NSHomeDirectory()),
			]
		}
	}

	/// `ccc` from the newest Devora bundle in /Applications, launched through a login shell so Claude Code is on PATH.
	private static func cccCommand() -> String {
		let bundles = (try? FileManager.default.contentsOfDirectory(atPath: "/Applications")) ?? []
		let ccc = bundles.filter { $0.hasPrefix("Devora") && $0.hasSuffix(".app") }.sorted()
			.map { "/Applications/\($0)/Contents/Resources/bundled-apps/ccc" }
			.first { FileManager.default.isExecutableFile(atPath: $0) }
		return "/bin/zsh -lc \(ccc ?? "ccc")"
	}

	/// The host's chords, routed before any terminal sees the key (ADR-003).
	func route(_ event: NSEvent) -> Bool {
		let flags = event.modifierFlags.intersection([.control, .command, .shift, .option])
		let key = event.charactersIgnoringModifiers?.lowercased() ?? ""
		if flags == [.command, .shift], key == "l" {
			routedChords.append("cmd-shift-l")
			togglePalette()
			return true
		}
		if flags == .control, key == "w" {
			pendingCtrlW = true
			routedChords.append("ctrl-w")
			return true
		}
		let direction = ["h": -1, "l": 1, "j": 1, "k": -1][key]
		if let direction, flags == .control || (pendingCtrlW && flags.isEmpty) {
			routedChords.append((pendingCtrlW ? "ctrl-w " : "ctrl-") + key)
			pendingCtrlW = false
			move(direction)
			return true
		}
		pendingCtrlW = false
		return false
	}

	func move(_ delta: Int) {
		focus = min(max(-1, focus + delta), sessions.count - 1)
	}

	func togglePalette() {
		let next: TerminalPalette = TerminalRuntime.shared.palette == .dark ? .light : .dark
		TerminalRuntime.shared.apply(next)
		status = "palette: \(next == .dark ? "dark" : "light")"
	}
}

public struct AppRoot: View {
	@State private var store = SpikeStore()

	public init() {}

	public var body: some View {
		VStack(spacing: 0) {
			HStack(spacing: 0) {
				NativePane(store: store)
					.frame(width: 260)
				Divider()
				TerminalGrid(store: store)
			}
			Divider()
			Text(store.status).font(.caption).padding(4).frame(maxWidth: .infinity, alignment: .leading)
		}
		.frame(minWidth: 1200, minHeight: 700)
		.task {
			if let output = LaunchOptions.value("--selftest") { await SelfTest.run(store: store, output: output) }
			if let output = LaunchOptions.value("--bench") { await Bench.run(store: store, output: output) }
		}
	}
}

private struct TerminalGrid: View {
	let store: SpikeStore

	var body: some View {
		let sessions = store.sessions
		let columns = max(1, sessions.count <= 3 ? sessions.count : Int(ceil(Double(sessions.count).squareRoot())))
		let rows = stride(from: 0, to: sessions.count, by: columns).map { Array(sessions[$0..<min($0 + columns, sessions.count)]) }
		Grid(horizontalSpacing: 1, verticalSpacing: 1) {
			ForEach(rows, id: \.first?.id) { row in
				GridRow {
					ForEach(row) { session in
						let index = sessions.firstIndex { $0 === session } ?? 0
						TerminalPane(
							session: session, isFocused: store.focus == index,
							router: { store.route($0) }, onFocus: { store.focus = index })
						.overlay(alignment: .top) {
							Rectangle().fill(store.focus == index ? Color.accentColor : .clear).frame(height: 2)
						}
					}
				}
			}
		}
		.background(Color.black)
	}
}

/// A plain SwiftUI pane beside the terminals: what Sapphire can read back from each one.
private struct NativePane: View {
	let store: SpikeStore
	@FocusState private var focused: Bool

	var body: some View {
		TimelineView(.periodic(from: .now, by: 1)) { _ in
			VStack(alignment: .leading, spacing: 10) {
				Text("Native pane").font(.headline)
				Text(store.focus == -1 ? "focused: Ctrl-l to go back" : "Ctrl-h/l, Ctrl-w h/l move focus; Cmd-Shift-L toggles the palette")
					.font(.caption).foregroundStyle(.secondary)
				ForEach(Array(store.sessions.enumerated()), id: \.element.id) { index, session in
					VStack(alignment: .leading, spacing: 2) {
						Text("\(index): \(session.command ?? "login shell")").font(.caption.bold()).lineLimit(1)
						Text("title: \(session.title)").lineLimit(1)
						Text("cwd (kernel): \(session.currentDirectory ?? "-")").lineLimit(1)
						Text("tty: \(session.ttyName ?? "-"), fg pid: \(session.foregroundPID.map(String.init) ?? "-")")
						if let code = session.exitCode { Text("exited: \(code)") }
					}
					.font(.caption.monospaced())
				}
				Spacer()
			}
		}
		.padding(10)
		.frame(maxHeight: .infinity, alignment: .top)
		.background(store.focus == -1 ? Color.accentColor.opacity(0.12) : Color.clear)
		.focusable()
		.focusEffectDisabled()
		.focused($focused)
		.onKeyPress(phases: .down) { press in
			if press.modifiers.contains(.control), press.key.character == "l" {
				store.focus = 0
				return .handled
			}
			return .ignored
		}
		.onChange(of: store.focus, initial: true) { _, now in focused = now == -1 }
	}
}

// MARK: - Scripted checks

/// Process memory as Activity Monitor's "Memory" column reports it.
func physFootprint() -> UInt64 {
	var info = task_vm_info_data_t()
	var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
	let result = withUnsafeMutablePointer(to: &info) {
		$0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
	}
	return result == KERN_SUCCESS ? info.phys_footprint : 0
}

/// GPU memory this process has allocated on the system device (libghostty's renderer uses the same device).
func gpuAllocated() -> Int {
	MTLCreateSystemDefaultDevice()?.currentAllocatedSize ?? 0
}

func writeJSON(_ value: some Encodable, to path: String) {
	let encoder = JSONEncoder()
	encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
	try? encoder.encode(value).write(to: URL(filePath: path))
}

/// Captures this app's own window with ScreenCaptureKit (no Screen Recording permission is needed for one's own windows).
@MainActor
func snapshot(to path: String) async {
	do {
		let content = try await SCShareableContent.currentProcess
		guard let window = content.windows.first(where: { $0.title == "libghostty spike" && $0.windowLayer == 0 }) else { return }
		let filter = SCContentFilter(desktopIndependentWindow: window)
		let config = SCStreamConfiguration()
		config.width = Int(window.frame.width * 2)
		config.height = Int(window.frame.height * 2)
		let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)
		let rep = NSBitmapImageRep(cgImage: image)
		try rep.representation(using: .png, properties: [:])?.write(to: URL(filePath: path))
	} catch {
		NSLog("snapshot failed: %@", String(describing: error))
	}
}

enum SelfTest {
	struct Result: Encodable {
		var zshCwdAfterCd: String?
		var zshScreenHasMarker: Bool
		var nvimScreenHead: String
		var cccScreenHead: String
		var keyWindowFound: Bool
		var firstResponderBeforeChords: String
		var routedChords: [String]
		var focusAfterCtrlL: Int
		var focusAfterCtrlWH: Int
		var scrollbackLines: Int
		var ttys: [String]
	}

	@MainActor
	static func run(store: SpikeStore, output: String) async {
		try? await Task.sleep(for: .seconds(4))
		let zsh = store.sessions[0]
		store.focus = 0
		try? await Task.sleep(for: .milliseconds(300))
		zsh.typeLine("cd /tmp && echo SAPPHIRE-$((40+2)) && seq 1 3000")
		try? await Task.sleep(for: .seconds(2))
		let cwd = zsh.currentDirectory

		// Real key events through the window: the host must take Ctrl-l (zsh would clear the screen) and Ctrl-w l.
		let window = NSApp.keyWindow ?? NSApp.windows.first { $0.title == "libghostty spike" }
		window?.makeKeyAndOrderFront(nil)
		let responder = "\(window?.firstResponder.map { String(describing: type(of: $0)) } ?? "nil"), key window: \(window?.isKeyWindow ?? false), app active: \(NSApp.isActive)"
		func key(_ chars: String, _ flags: NSEvent.ModifierFlags, code: UInt16, ignoringModifiers: String? = nil) {
			for type in [NSEvent.EventType.keyDown, .keyUp] {
				let event = NSEvent.keyEvent(
					with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
					windowNumber: window?.windowNumber ?? 0, context: nil, characters: chars,
					charactersIgnoringModifiers: ignoringModifiers ?? chars, isARepeat: false, keyCode: code)!
				if event.modifierFlags.contains(.command) { NSApp.sendEvent(event) } else { window?.sendEvent(event) }
			}
		}
		store.focus = 0
		try? await Task.sleep(for: .milliseconds(300))
		key("\u{0c}", .control, code: 37, ignoringModifiers: "l")  // Ctrl-l: to the nvim pane
		try? await Task.sleep(for: .milliseconds(300))
		let focusAfterCtrlL = store.focus
		key("\u{17}", .control, code: 13, ignoringModifiers: "w")  // Ctrl-w
		key("h", [], code: 4)  // then h: back to zsh
		try? await Task.sleep(for: .milliseconds(300))
		key("L", [.command, .shift], code: 37)  // palette to light
		try? await Task.sleep(for: .seconds(1))
		await snapshot(to: output + ".light.png")
		key("L", [.command, .shift], code: 37)  // and back
		try? await Task.sleep(for: .seconds(1))
		await snapshot(to: output + ".dark.png")

		let zshText = zsh.allText() ?? ""
		let result = Result(
			zshCwdAfterCd: cwd,
			zshScreenHasMarker: zshText.contains("SAPPHIRE-42"),
			nvimScreenHead: String((store.sessions[1].allText() ?? "").prefix(200)),
			cccScreenHead: String((store.sessions[2].allText() ?? "").prefix(400)),
			keyWindowFound: window != nil,
			firstResponderBeforeChords: responder,
			routedChords: store.routedChords,
			focusAfterCtrlL: focusAfterCtrlL,
			focusAfterCtrlWH: store.focus,
			scrollbackLines: zshText.split(separator: "\n").count,
			ttys: store.sessions.compactMap(\.ttyName))
		writeJSON(result, to: output)
		NSApp.terminate(nil)
	}
}

enum Bench {
	struct Sample: Encodable {
		var terminals: Int
		var footprintMB: Double
		var gpuAllocatedMB: Double
	}

	struct Throughput: Encodable {
		var terminals: Int
		var lines: Int
		var secondsUntilAllExited: Double
	}

	struct Result: Encodable {
		var baseline: Sample
		var samples: [Sample]
		var throughput: [Throughput]
	}

	@MainActor
	static func measure(_ n: Int) -> Sample {
		Sample(terminals: n, footprintMB: Double(physFootprint()) / 1_048_576, gpuAllocatedMB: Double(gpuAllocated()) / 1_048_576)
	}

	@MainActor
	static func run(store: SpikeStore, output: String) async {
		try? await Task.sleep(for: .seconds(3))
		let baseline = measure(0)
		var samples: [Sample] = []
		for n in [1, 4, 8] {
			store.sessions = (0..<n).map { _ in TerminalSession(command: nil, workingDirectory: NSHomeDirectory()) }
			try? await Task.sleep(for: .seconds(4))
			samples.append(measure(n))
			store.sessions = []
			try? await Task.sleep(for: .seconds(2))
		}

		// Output throughput: each terminal prints `lines` lines and exits; time until all report their exit.
		var throughput: [Throughput] = []
		let lines = 1_000_000
		for n in [1, 4] {
			store.sessions = (0..<n).map { _ in TerminalSession(command: "/usr/bin/seq 1 \(lines)", workingDirectory: "/tmp") }
			let start = ContinuousClock.now
			while store.sessions.contains(where: { $0.exitCode == nil && !$0.isClosed }), ContinuousClock.now - start < .seconds(120) {
				try? await Task.sleep(for: .milliseconds(20))
			}
			let elapsed = store.sessions.compactMap(\.runtime).max() ?? (ContinuousClock.now - start)
			throughput.append(Throughput(terminals: n, lines: lines, secondsUntilAllExited: Double(elapsed.components.seconds) + Double(elapsed.components.attoseconds) / 1e18))
			store.sessions = []
			try? await Task.sleep(for: .seconds(2))
		}
		writeJSON(Result(baseline: baseline, samples: samples, throughput: throughput), to: output)
		NSApp.terminate(nil)
	}
}
