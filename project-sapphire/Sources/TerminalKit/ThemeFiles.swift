import Darwin
import Foundation

/// The two files the config's theme names (`theme = light:…,dark:…`), both holding the active palette: libghostty tells a
/// program of the scheme (mode 2031) only through a conditional theme, which needs two names, while Sapphire picks the
/// variant itself, so neither scheme may draw other colors (TerminalKit's CLAUDE.md).
///
/// libghostty reads them again whenever it replays a surface's config, so they live as long as the app: in a directory
/// of this process's own, removed at quit; one left by a process that is gone (a crash) is removed at the next launch.
struct ThemeFiles {
	static let prefix = "sapphire-ghostty-themes-"

	let directory: URL
	var light: URL { directory.appending(path: "light") }
	var dark: URL { directory.appending(path: "dark") }

	/// This process's directory under `parent`, after removing those of processes that are gone.
	static func forThisProcess(in parent: URL) -> ThemeFiles {
		removeAbandoned(in: parent)
		let files = ThemeFiles(directory: parent.appending(path: "\(prefix)\(getpid())"))
		try? FileManager.default.createDirectory(at: files.directory, withIntermediateDirectories: true)
		return files
	}

	/// Writes `palette` to both files, each replaced whole: a surface's replay may read them at any moment.
	func write(_ palette: TerminalPalette) throws {
		let text = Data(palette.configLines.joined(separator: "\n").utf8)
		try text.write(to: light, options: .atomic)
		try text.write(to: dark, options: .atomic)
	}

	func remove() {
		try? FileManager.default.removeItem(at: directory)
	}

	private static func removeAbandoned(in parent: URL) {
		let names = (try? FileManager.default.contentsOfDirectory(atPath: parent.path)) ?? []
		for name in names where name.hasPrefix(prefix) {
			guard let pid = pid_t(name.dropFirst(prefix.count)), !isRunning(pid) else { continue }
			try? FileManager.default.removeItem(at: parent.appending(path: name))
		}
	}

	/// Signal 0 checks that the process exists without sending it anything.
	private static func isRunning(_ pid: pid_t) -> Bool {
		kill(pid, 0) == 0 || errno == EPERM
	}
}

extension TerminalPalette {
	/// The palette as Ghostty config lines, the format of a theme file.
	var configLines: [String] {
		[
			"background = \(background)",
			"foreground = \(foreground)",
			"cursor-color = \(cursor)",
		] + ansi.enumerated().map { "palette = \($0.offset)=\($0.element)" }
	}
}
