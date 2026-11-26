import AppKit
import Foundation
import GhosttyKit

/// Colors for every terminal: one variant of a palette family, light or dark, after the app's effective appearance.
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

	/// The palettes a user picks from (the `terminal.palette` setting): known palettes, kept as their authors drew them,
	/// except that a text color under 4.5:1 against its page was darkened (light) or lightened (dark) in its hue until it
	/// reached 4.5:1. In each light variant "white" and "bright white" stay light, and in each dark one "black" stays
	/// dark: programs use them as backgrounds.
	public enum Family: Sendable, CaseIterable {
		case selenized
		case catppuccin
		case gruvbox

		public var light: TerminalPalette {
			switch self {
			case .selenized: .selenizedLight
			case .catppuccin: .catppuccinLatte
			case .gruvbox: .gruvboxLight
			}
		}

		public var dark: TerminalPalette {
			switch self {
			case .selenized: .selenizedDark
			case .catppuccin: .catppuccinMacchiato
			case .gruvbox: .gruvboxDark
			}
		}
	}

	static let selenizedLight = TerminalPalette(
		background: "#fbf3db", foreground: "#53676d", cursor: "#3a4d53",
		ansi: ["#ece3cc", "#d2212d", "#3f7e00", "#886c00", "#006fcf", "#c0388b", "#007d73", "#909995", "#7c6e47", "#cc1729", "#3c7e00", "#886b00", "#006dce", "#be3b8b", "#007d73", "#3a4d53"])
	static let selenizedDark = TerminalPalette(
		background: "#103c48", foreground: "#adbcbc", cursor: "#cad8d9",
		ansi: ["#184956", "#fb7771", "#75b938", "#dbb32d", "#5ca2f8", "#f275be", "#41c7b9", "#88a4ab", "#65a9bd", "#ff746b", "#84c747", "#ebc13d", "#58a3ff", "#ff84cd", "#53d6c7", "#cad8d9"])
	static let catppuccinLatte = TerminalPalette(
		background: "#eff1f5", foreground: "#4c4f69", cursor: "#dc8a78",
		ansi: ["#5c5f77", "#d20f39", "#327d22", "#996114", "#1862f5", "#8839ef", "#137a80", "#acb0be", "#6a6d82", "#b50d31", "#29681c", "#7f5111", "#0a4ed6", "#7418ec", "#106569", "#bcc0cc"])
	static let catppuccinMacchiato = TerminalPalette(
		background: "#24273a", foreground: "#cad3f5", cursor: "#f4dbd6",
		ansi: ["#494d64", "#ed8796", "#a6da95", "#eed49f", "#8aadf4", "#c6a0f6", "#8bd5ca", "#b8c0e0", "#898ea3", "#ee99a0", "#b7e3a7", "#f5e0b3", "#9dbaf7", "#d2b4f8", "#91d7e3", "#cad3f5"])
	static let gruvboxLight = TerminalPalette(
		background: "#fbf1c7", foreground: "#3c3836", cursor: "#3c3836",
		ansi: ["#ebdbb2", "#cc241d", "#727113", "#8f6616", "#3d7679", "#a65278", "#4d774f", "#7c6f64", "#786b5e", "#9d0006", "#74700d", "#976311", "#076678", "#8f3f71", "#417957", "#3c3836"])
	static let gruvboxDark = TerminalPalette(
		background: "#282828", foreground: "#ebdbb2", cursor: "#ebdbb2",
		ansi: ["#3c3836", "#e8655f", "#98971a", "#d79921", "#509a9d", "#bd7b99", "#689d6a", "#a89984", "#9a8c7e", "#fb533f", "#b8bb26", "#fabd2f", "#83a598", "#d3869b", "#8ec07c", "#fbf1c7"])
}

/// The process-wide libghostty app: initialization, configuration and the runtime callbacks.
///
/// Sapphire never loads the user's Ghostty config; everything comes from `configText`, generated from Devora's own settings.
@MainActor
public final class TerminalRuntime {
	public static let shared = TerminalRuntime()

	/// Where libghostty's complaints about the generated config go: the app sets its log before the first terminal starts.
	public static var logWarning: (String) -> Void = { NSLog("%@", $0) }

	let app: ghostty_app_t
	/// The chosen palette's light and dark variants; the terminals draw in the one for the app's appearance (`dark`).
	public private(set) var palettes = (light: TerminalPalette.Family.selenized.light, dark: TerminalPalette.Family.selenized.dark)
	/// Whether the terminals draw in the dark variant.
	public private(set) var dark = true
	/// The variant the terminals draw in.
	private var palette: TerminalPalette { dark ? palettes.dark : palettes.light }
	/// The size terminals start with, in points (Ember's default, which `Ctrl+=` returns to).
	public nonisolated static let defaultFontSize: Double = 15
	/// The font size of every terminal, in points.
	public private(set) var fontSize = defaultFontSize
	private var config: ghostty_config_t?
	private let themes = ThemeFiles.forThisProcess(in: FileManager.default.temporaryDirectory)
	/// The live terminals, each told the scheme (`ghostty_surface_set_color_scheme`) when it's made and on every switch.
	private let views = NSHashTable<TerminalView>.weakObjects()

	private init() {
		// libghostty parses argv for its own CLI actions; give it only the program name so Sapphire's flags stay ours.
		var argv: [UnsafeMutablePointer<CChar>?] = [strdup(CommandLine.arguments.first ?? "sapphire"), nil]
		guard ghostty_init(1, &argv) == GHOSTTY_SUCCESS else { fatalError("ghostty_init failed") }

		Self.write(palettes.dark, to: themes)
		let config = Self.makeConfig(themes: themes, fontSize: Self.defaultFontSize)
		NotificationCenter.default.addObserver(forName: NSApplication.willTerminateNotification, object: nil, queue: .main) { _ in
			MainActor.assumeIsolated { TerminalRuntime.shared.themes.remove() }
		}
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

	/// Applies the palette's variant for the appearance to every terminal at once, through a regenerated config, and tells
	/// each terminal's program of the scheme if it asked (mode 2031).
	public func apply(light: TerminalPalette, dark: TerminalPalette, isDark: Bool) {
		guard light != palettes.light || dark != palettes.dark || isDark != self.dark else { return }
		palettes = (light, dark)
		self.dark = isDark
		reconfigure()
	}

	/// Resizes every terminal's font at once, through a regenerated config; new terminals start with it too.
	public func setFontSize(_ points: Double) {
		guard points != fontSize else { return }
		fontSize = points
		reconfigure()
	}

	/// In this order, since setting a surface's scheme replays its config, which reads the theme files then: the files,
	/// the config, then the scheme of each terminal.
	private func reconfigure() {
		Self.write(palette, to: themes)
		let config = Self.makeConfig(themes: themes, fontSize: fontSize)
		ghostty_app_update_config(app, config)
		if let old = self.config { ghostty_config_free(old) }
		self.config = config
		for view in views.allObjects { tellScheme(view) }
	}

	private static func write(_ palette: TerminalPalette, to themes: ThemeFiles) {
		do { try themes.write(palette) } catch { Self.logWarning("couldn't write the terminal colors: \(error)") }
	}

	/// Starts telling a newly made terminal the scheme.
	func register(_ view: TerminalView) {
		views.add(view)
		tellScheme(view)
	}

	private func tellScheme(_ view: TerminalView) {
		guard let surface = view.surface else { return }
		ghostty_surface_set_color_scheme(surface, dark ? GHOSTTY_COLOR_SCHEME_DARK : GHOSTTY_COLOR_SCHEME_LIGHT)
	}

	/// libghostty asks for this (`reload_config`, soft) when a surface's scheme changes, and replays the surface's config
	/// with the new scheme only once it gets it, as Ghostty's own app does.
	private func reloadConfig(_ surface: ghostty_surface_t) {
		guard let config else { return }
		ghostty_surface_update_config(surface, config)
	}

	public func setAppFocus(_ focused: Bool) {
		ghostty_app_set_focus(app, focused)
	}

	/// libghostty has no setter API: a config is built by loading a file, so write ours to a temporary file.
	/// The colors are the theme files', both the active variant; never a scheme on the app (`ghostty_app_set_color_scheme`),
	/// since a terminal made while the app's scheme differs from the config's default gets a replayed copy of the config
	/// that drops its environment and command (TerminalKit's CLAUDE.md).
	private static func makeConfig(themes: ThemeFiles, fontSize: Double) -> ghostty_config_t {
		let lines = [
			// Sapphire routes its own chords before the terminal (ADR-003), and binds only copy here.
			"keybind = clear",
			"keybind = super+c=copy_to_clipboard",
			"font-size = \(fontSize)",
			"window-padding-x = 4",
			"window-padding-y = 4",
			"confirm-close-surface = false",
			"shell-integration = none",
			// Not xterm-ghostty: its terminfo lives in Ghostty's resources dir, which Sapphire doesn't ship yet.
			"term = xterm-256color",
			"theme = light:\(themes.light.path),dark:\(themes.dark.path)",
		]
		let url = FileManager.default.temporaryDirectory.appending(path: "sapphire-ghostty-\(UUID().uuidString).conf")
		try? lines.joined(separator: "\n").write(to: url, atomically: true, encoding: .utf8)
		defer { try? FileManager.default.removeItem(at: url) }

		guard let config = ghostty_config_new() else { fatalError("ghostty_config_new failed") }
		url.path.withCString { ghostty_config_load_file(config, $0) }
		ghostty_config_finalize(config)
		for i in 0..<ghostty_config_diagnostics_count(config) {
			let diagnostic = ghostty_config_get_diagnostic(config, i)
			logWarning("ghostty config: \(String(cString: diagnostic.message))")
		}
		return config
	}

	/// Actions can arrive on any thread, and their C strings are only valid during the call: copy, then hop to the main thread.
	private nonisolated static func handle(_ action: ghostty_action_s, target: ghostty_target_s) -> Bool {
		guard target.tag == GHOSTTY_TARGET_SURFACE, let surface = target.target.surface,
		      let userdata = ghostty_surface_userdata(surface)
		else { return false }
		let view = Unmanaged<TerminalView>.fromOpaque(userdata).takeUnretainedValue()
		if action.tag == GHOSTTY_ACTION_RELOAD_CONFIG {
			guard action.action.reload_config.soft else { return false }
			// It comes from `ghostty_surface_set_color_scheme`, on the main thread, and is answered before that returns.
			if Thread.isMainThread {
				MainActor.assumeIsolated { TerminalRuntime.shared.reloadConfig(surface) }
			} else {
				DispatchQueue.main.async { MainActor.assumeIsolated { view.surface.map { TerminalRuntime.shared.reloadConfig($0) } } }
			}
			return true
		}
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
