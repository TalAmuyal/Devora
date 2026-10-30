/// What a terminal runs: the core's terminal spec, in TerminalKit's own terms.
public struct TerminalLaunch: Sendable, Equatable {
	public var program: String
	public var args: [String]
	public var cwd: String
	/// Variables to set on top of the inherited environment.
	public var env: [String: String]
	/// Inherited variables to remove.
	public var envRemove: [String]

	public init(program: String, args: [String], cwd: String, env: [String: String], envRemove: [String]) {
		self.program = program
		self.args = args
		self.cwd = cwd
		self.env = env
		self.envRemove = envRemove
	}

	/// The command line libghostty runs, as `bash -c "exec -l <command>"`.
	///
	/// libghostty only takes a surface's command as one shell string, and has no way to remove an inherited variable from one surface (it snapshots the process environment once, at `ghostty_init`), so removals go through `env -u`.
	public var shellCommand: String {
		let removals = envRemove.isEmpty ? [] : ["/usr/bin/env"] + envRemove.flatMap { ["-u", $0] }
		return (removals + [program] + args).map(Self.quote).joined(separator: " ")
	}

	/// POSIX single quoting: every character is literal, and a `'` is closed, escaped and reopened.
	static func quote(_ word: String) -> String {
		"'" + word.replacing("'", with: #"'\''"#) + "'"
	}
}
