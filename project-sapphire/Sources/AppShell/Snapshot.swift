import AppKit
import ScreenCaptureKit
import Store

/// Launch arguments come in `--flag value` pairs: a bare argument makes AppKit open it as a file, and SwiftUI then opens no window.
enum LaunchOptions {
	static func value(_ flag: String) -> String? {
		let args = CommandLine.arguments
		guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
		return args[i + 1]
	}
}

/// `--snapshot <path>`: runs a command in the first shell tab, if there is one, captures the window to `<path>` (PNG) and the active tab's text to `<path>.txt`, then quits.
@MainActor
enum Snapshot {
	static func run(store: Store, terminals: Terminals, to path: String) async {
		try? await Task.sleep(for: .seconds(3))
		let window = store.app.window
		if let shell = window.tabs.first(where: { $0.session.kind == .shell }) {
			terminals.session(for: shell)?.typeLine("echo SAPPHIRE-$((40+2))")
		}
		try? await Task.sleep(for: .seconds(1))
		await captureWindow(to: path, store: store)
		let active = window.tabs.first { $0.id == window.activeTab }
		let text = active.flatMap { terminals.session(for: $0)?.allText() } ?? ""
		try? text.write(toFile: path + ".txt", atomically: true, encoding: .utf8)
		NSApp.terminate(nil)
	}

	/// ScreenCaptureKit needs no Screen Recording permission for the process's own windows.
	static func captureWindow(to path: String, store: Store) async {
		do {
			let content = try await SCShareableContent.currentProcess
			guard let window = content.windows.first(where: { $0.title?.hasPrefix("Devora Sapphire") == true && $0.windowLayer == 0 })
			else {
				store.logWarning("snapshot: no window found")
				return
			}
			let config = SCStreamConfiguration()
			config.width = Int(window.frame.width * 2)
			config.height = Int(window.frame.height * 2)
			let image = try await SCScreenshotManager.captureImage(
				contentFilter: SCContentFilter(desktopIndependentWindow: window), configuration: config)
			try NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])?.write(to: URL(filePath: path))
		} catch {
			store.logWarning("snapshot failed: \(error)")
		}
	}
}
