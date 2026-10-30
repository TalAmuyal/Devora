import AppKit
import Foundation
import GhosttyKit

/// Colors for every terminal, a stand-in for `devora-theme` tokens.
public struct TerminalPalette: Sendable, Equatable {
	public var background: String
	public var foreground: String
	public var cursor: String
	/// The 16 ANSI colors, as `#rrggbb`.
	public var ansi: [String]

	public init(background: String, foreground: String, cursor: String, ansi: [String]) {
		self.background = background
		self.foreground = foreground
		self.cursor = cursor
		self.ansi = ansi
	}

	public static let dark = TerminalPalette(
		background: "#24273a", foreground: "#cad3f5", cursor: "#f4dbd6",
		ansi: ["#494d64", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#f5bde6", "#8bd5ca", "#b8c0e0",
		       "#5b6078", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#f5bde6", "#8bd5ca", "#a5adcb"])

	public static let light = TerminalPalette(
		background: "#eff1f5", foreground: "#4c4f69", cursor: "#dc8a78",
		ansi: ["#5c5f77", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#acb0be",
		       "#6c6f85", "#d20f39", "#40a02b", "#df8e1d", "#1e66f5", "#ea76cb", "#179299", "#bcc0cc"])
}

/// The process-wide libghostty app: initialization, configuration and the runtime callbacks.
///
/// Sapphire never loads the user's Ghostty config; everything comes from `configText`, generated from Devora's own settings.
@MainActor
public final class TerminalRuntime {
	public static let shared = TerminalRuntime()

	let app: ghostty_app_t
	public private(set) var palette = TerminalPalette.dark
	private var config: ghostty_config_t?

	private init() {
		// libghostty parses argv for its own CLI actions; give it only the program name so Sapphire's flags stay ours.
		var argv: [UnsafeMutablePointer<CChar>?] = [strdup(CommandLine.arguments.first ?? "sapphire"), nil]
		guard ghostty_init(1, &argv) == GHOSTTY_SUCCESS else { fatalError("ghostty_init failed") }

		let config = Self.makeConfig(palette: .dark)
		var runtime = Self.runtimeCallbacks()
		guard let app = ghostty_app_new(&runtime, config) else { fatalError("ghostty_app_new failed") }
		self.app = app
		self.config = config
		ghostty_app_set_focus(app, NSApp?.isActive ?? true)
	}

	/// libghostty calls these from its renderer and IO threads, so they must be built outside any actor.
	/// Closures written in a `@MainActor` context inherit its isolation, and Swift 6's runtime check then aborts the app the first time a renderer thread calls one.
	private nonisolated static func runtimeCallbacks() -> ghostty_runtime_config_s {
		ghostty_runtime_config_s(
			userdata: nil,
			supports_selection_clipboard: false,
			wakeup_cb: { _ in
				DispatchQueue.main.async { MainActor.assumeIsolated { TerminalRuntime.shared.tick() } }
			},
			action_cb: { _, target, action in TerminalRuntime.handle(action, target: target) },
			read_clipboard_cb: { _, _, _, _, _, _ in GHOSTTY_CLIPBOARD_READ_UNSUPPORTED },
			confirm_read_clipboard_cb: { _, _, _, _ in },
			write_clipboard_cb: { _, location, contents, count, _ in
				guard location == GHOSTTY_CLIPBOARD_STANDARD, let contents else { return }
				for item in UnsafeBufferPointer(start: contents, count: count)
				where item.mime.map({ String(cString: $0) }) == "text/plain" {
					guard let data = item.data else { continue }
					let text = String(decoding: UnsafeRawBufferPointer(start: data, count: item.len), as: UTF8.self)
					DispatchQueue.main.async {
						NSPasteboard.general.clearContents()
						NSPasteboard.general.setString(text, forType: .string)
					}
				}
			},
			close_surface_cb: { userdata, _ in
				guard let userdata else { return }
				let view = Unmanaged<TerminalView>.fromOpaque(userdata).takeUnretainedValue()
				DispatchQueue.main.async { MainActor.assumeIsolated { view.session.markClosed() } }
			}
		)
	}

	func tick() {
		ghostty_app_tick(app)
	}

	/// Applies new colors to every terminal at once, through a regenerated config.
	public func apply(_ palette: TerminalPalette) {
		self.palette = palette
		let config = Self.makeConfig(palette: palette)
		ghostty_app_update_config(app, config)
		if let old = self.config { ghostty_config_free(old) }
		self.config = config
	}

	public func setAppFocus(_ focused: Bool) {
		ghostty_app_set_focus(app, focused)
	}

	/// libghostty has no setter API: a config is built by loading a file, so write ours to a temporary file.
	private static func makeConfig(palette: TerminalPalette) -> ghostty_config_t {
		let lines = [
			// Sapphire routes its own chords before the terminal (ADR-003), and binds only copy here.
			"keybind = clear",
			"keybind = super+c=copy_to_clipboard",
			"font-size = 13",
			"window-padding-x = 4",
			"window-padding-y = 4",
			"confirm-close-surface = false",
			"shell-integration = none",
			// Not xterm-ghostty: its terminfo lives in Ghostty's resources dir, which Sapphire doesn't ship yet.
			"term = xterm-256color",
			"background = \(palette.background)",
			"foreground = \(palette.foreground)",
			"cursor-color = \(palette.cursor)",
		] + palette.ansi.enumerated().map { "palette = \($0.offset)=\($0.element)" }
		let url = FileManager.default.temporaryDirectory.appending(path: "sapphire-ghostty-\(UUID().uuidString).conf")
		try? lines.joined(separator: "\n").write(to: url, atomically: true, encoding: .utf8)
		defer { try? FileManager.default.removeItem(at: url) }

		guard let config = ghostty_config_new() else { fatalError("ghostty_config_new failed") }
		url.path.withCString { ghostty_config_load_file(config, $0) }
		ghostty_config_finalize(config)
		for i in 0..<ghostty_config_diagnostics_count(config) {
			let diagnostic = ghostty_config_get_diagnostic(config, i)
			NSLog("ghostty config: %@", String(cString: diagnostic.message))
		}
		return config
	}

	/// Actions can arrive on any thread, and their C strings are only valid during the call: copy, then hop to the main thread.
	private nonisolated static func handle(_ action: ghostty_action_s, target: ghostty_target_s) -> Bool {
		guard target.tag == GHOSTTY_TARGET_SURFACE, let surface = target.target.surface,
		      let userdata = ghostty_surface_userdata(surface)
		else { return false }
		let view = Unmanaged<TerminalView>.fromOpaque(userdata).takeUnretainedValue()
		let update: (@MainActor @Sendable (TerminalSession) -> Void)?
		switch action.tag {
		case GHOSTTY_ACTION_SET_TITLE:
			let title = action.action.set_title.title.map { String(cString: $0) } ?? ""
			update = { $0.title = title }
		case GHOSTTY_ACTION_PWD:
			let pwd = action.action.pwd.pwd.map { String(cString: $0) }
			update = { $0.reportedPwd = pwd }
		case GHOSTTY_ACTION_SHOW_CHILD_EXITED:
			let code = Int32(bitPattern: action.action.child_exited.exit_code)
			update = { $0.markExited(code: code) }
		default:
			update = nil
		}
		guard let update else { return false }
		DispatchQueue.main.async { MainActor.assumeIsolated { update(view.session) } }
		return true
	}
}
