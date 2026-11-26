import Foundation
import Testing
@testable import TerminalKit

/// The theme files live as long as their app: a launch removes those a gone process left, and never another running
/// instance's.
struct ThemeFilesTests {
	private let parent = FileManager.default.temporaryDirectory.appending(path: "theme-files-tests-\(UUID().uuidString)")

	private func makeDirectory(for pid: pid_t) throws -> URL {
		let directory = parent.appending(path: "\(ThemeFiles.prefix)\(pid)")
		try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
		return directory
	}

	/// The pid of a process that has exited and been reaped.
	private func exitedPid() throws -> pid_t {
		let process = Process()
		process.executableURL = URL(fileURLWithPath: "/usr/bin/true")
		try process.run()
		process.waitUntilExit()
		return process.processIdentifier
	}

	@Test func aLaunchRemovesTheFilesOfAGoneProcessOnly() throws {
		defer { try? FileManager.default.removeItem(at: parent) }
		let gone = try makeDirectory(for: try exitedPid())
		// launchd: running, though not this user's (signal 0 answers EPERM).
		let running = try makeDirectory(for: 1)
		let unrelated = parent.appending(path: "something-else")
		try FileManager.default.createDirectory(at: unrelated, withIntermediateDirectories: true)

		let files = ThemeFiles.forThisProcess(in: parent)

		#expect(!FileManager.default.fileExists(atPath: gone.path))
		#expect(FileManager.default.fileExists(atPath: running.path))
		#expect(FileManager.default.fileExists(atPath: unrelated.path))
		#expect(files.directory.lastPathComponent == "\(ThemeFiles.prefix)\(getpid())")
		#expect(FileManager.default.fileExists(atPath: files.directory.path))
	}

	@Test func bothFilesHoldTheLastPaletteWritten() throws {
		defer { try? FileManager.default.removeItem(at: parent) }
		let files = ThemeFiles.forThisProcess(in: parent)
		try files.write(TerminalPalette.Family.gruvbox.light)
		try files.write(TerminalPalette.Family.gruvbox.dark)

		let expected = TerminalPalette.Family.gruvbox.dark.configLines.joined(separator: "\n")
		#expect(try String(contentsOf: files.light, encoding: .utf8) == expected)
		#expect(try String(contentsOf: files.dark, encoding: .utf8) == expected)

		files.remove()
		#expect(!FileManager.default.fileExists(atPath: files.directory.path))
	}
}
