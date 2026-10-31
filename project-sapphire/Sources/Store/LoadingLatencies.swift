/// How long the Workspace Hub's latest load took, phase by phase and workspace by workspace: the report "Save loading latencies" writes, in the shape of Ember's hub profiling report.
///
/// Times are milliseconds from the start of the listing; Ember's `render` phase and its per-repo timings have no counterpart here.
public struct LoadingLatencies: Encodable, Sendable {
	public struct Phase: Encodable, Sendable {
		public var startMs: Double
		public var durationMs: Double

		static let zero = Phase(startMs: 0, durationMs: 0)

		init(startMs: Double, durationMs: Double) {
			self.startMs = startMs
			self.durationMs = durationMs
		}

		init(from start: ContinuousClock.Instant, to end: ContinuousClock.Instant, origin: ContinuousClock.Instant) {
			self.init(startMs: (start - origin).milliseconds, durationMs: (end - start).milliseconds)
		}
	}

	public struct Phases: Encodable, Sendable {
		/// Zero when only the workspaces were listed, after a profile switch.
		public var listProfiles: Phase
		public var listWorkspaces: Phase
		/// Until the workspaces are listed; the statuses load after it.
		public var totalLoad: Phase
	}

	public struct StatusLatency: Encodable, Sendable {
		public var wsId: String
		public var repoCount: Int
		public var startMs: Double
		public var durationMs: Double
		public var error: String?
	}

	public var timestamp: String
	public var profileName: String
	public var profilePath: String
	public var workspaceCount: Int
	public var phases: Phases
	public var workspaceStatuses: [StatusLatency]
}

extension Duration {
	var milliseconds: Double {
		let (seconds, attoseconds) = components
		return Double(seconds) * 1000 + Double(attoseconds) / 1e15
	}
}
