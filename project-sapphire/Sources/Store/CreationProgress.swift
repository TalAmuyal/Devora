import CoreBindings

/// What a task creation's progress page shows, folded from the core's creation events, as Ember's `TaskCreationProgress` renders them: the steps with their states, the subprocess output, and the failure.
public struct CreationProgress: Equatable, Sendable {
	public enum StepState: Equatable, Sendable {
		case active
		case done
		case failed
	}

	public struct Step: Equatable, Sendable {
		public var label: String
		public var state: StepState

		public init(label: String, state: StepState) {
			self.label = label
			self.state = state
		}
	}

	/// The output keeps its latest lines only, so a chatty prepare-command can't grow it without bound.
	public static let outputLimit = 500

	public private(set) var steps: [Step] = []
	public private(set) var output: [String] = []
	public private(set) var failure: String?

	public init() {}

	public mutating func apply(_ event: CreationEvent) {
		switch event {
		case .step(let label):
			finishActiveStep(as: .done)
			steps.append(Step(label: label, state: .active))
		case .log(let line):
			output.append(line)
			if output.count > Self.outputLimit { output.removeFirst(output.count - Self.outputLimit) }
		case .done:
			finishActiveStep(as: .done)
		case .failed(let message):
			fail(message)
		case .cancelled:
			break
		}
	}

	/// Marks the active step failed and records `message`; the first failure is the one kept.
	public mutating func fail(_ message: String) {
		guard failure == nil else { return }
		finishActiveStep(as: .failed)
		failure = message
	}

	private mutating func finishActiveStep(as state: StepState) {
		if let last = steps.indices.last, steps[last].state == .active { steps[last].state = state }
	}
}

extension CreationEvent {
	/// Whether this event ends the creation's stream.
	var isLast: Bool {
		switch self {
		case .step, .log: false
		case .done, .failed, .cancelled: true
		}
	}
}

extension CreationSubscription: BatchSubscription {}
