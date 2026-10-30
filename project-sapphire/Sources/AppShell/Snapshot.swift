import AppKit
import ScreenCaptureKit
import TerminalKit

/// Launch arguments come in `--flag value` pairs: a bare argument makes AppKit open it as a file, and SwiftUI then opens no window.
enum LaunchOptions {
	static func value(_ flag: String) -> String? {
		let args = CommandLine.arguments
		guard let i = args.firstIndex(of: flag), i + 1 < args.count else { return nil }
		return args[i + 1]
	}
}

/// `--snapshot <path>`: runs a command in the shell, captures the window to `<path>` (PNG) and the shell's text to `<path>.txt`, then quits.
@MainActor
enum Snapshot {
	static func run(shell: TerminalSession, to path: String) async {
		try? await Task.sleep(for: .seconds(3))
		shell.typeLine("echo SAPPHIRE-$((40+2))")
		try? await Task.sleep(for: .seconds(1))
		await captureWindow(to: path)
		try? (shell.allText() ?? "").write(toFile: path + ".txt", atomically: true, encoding: .utf8)
		NSApp.terminate(nil)
	}

	/// ScreenCaptureKit needs no Screen Recording permission for the process's own windows.
	private static func captureWindow(to path: String) async {
		do {
			let content = try await SCShareableContent.currentProcess
			guard let window = content.windows.first(where: { $0.title?.hasPrefix("Devora Sapphire") == true && $0.windowLayer == 0 })
			else {
				NSLog("snapshot: no window found")
				return
			}
			let config = SCStreamConfiguration()
			config.width = Int(window.frame.width * 2)
			config.height = Int(window.frame.height * 2)
			let image = try await SCScreenshotManager.captureImage(
				contentFilter: SCContentFilter(desktopIndependentWindow: window), configuration: config)
			try NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:])?.write(to: URL(filePath: path))
		} catch {
			NSLog("snapshot failed: %@", String(describing: error))
		}
	}
}
