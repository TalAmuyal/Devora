import Foundation
import TerminalKit
import Testing

struct TerminalLaunchTests {
	/// Runs the command line the way libghostty does on macOS (minus `login`), and returns what the program printed.
	private func runAsGhosttyWould(_ launch: TerminalLaunch, environment: [String: String]) throws -> String {
		let process = Process()
		process.executableURL = URL(filePath: "/bin/bash")
		process.arguments = ["--noprofile", "--norc", "-c", "exec -l \(launch.shellCommand)"]
		process.environment = environment
		let output = Pipe()
		process.standardOutput = output
		try process.run()
		process.waitUntilExit()
		return String(decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
	}

	@Test func theProgramGetsItsArgumentsVerbatimWithoutTheRemovedVariables() throws {
		let launch = TerminalLaunch(
			program: "/bin/sh",
			args: ["-c", #"printf '[%s]\n' "$@"; echo "keep=${KEEP-unset} drop=${DROP-unset}""#, "sh", "two words", "it's", "$HOME", ""],
			cwd: "/", env: [:], envRemove: ["DROP"])

		let output = try runAsGhosttyWould(launch, environment: ["KEEP": "kept", "DROP": "inherited"])

		#expect(output == "[two words]\n[it's]\n[$HOME]\n[]\nkeep=kept drop=unset\n")
	}
}
