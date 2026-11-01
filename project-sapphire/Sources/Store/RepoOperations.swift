import CoreBindings
import Observation

/// An operation on a profile's repos, which streams its progress in the dialog that started it, as Ember's `driveCreationDialog` does.
public enum RepoOperationRequest: Equatable, Sendable {
	/// Adds a worktree of the repo at `sourceRepoPath` to the workspace, as `worktreeDirName` (`add_repo_to_workspace`).
	case addRepo(workspacePath: String, sourceRepoPath: String, worktreeDirName: String)
	/// Clones the repo at `url` into the profile's `repos/`, detached (`clone_repo_into_profile`).
	case cloneRepo(profilePath: String, url: String)
}

/// The workspace an Add Repo adds to, and the profile whose repos it offers: the workspace's own, whichever profile is selected.
public struct AddRepoTarget: Equatable, Sendable {
	public let workspacePath: String
	public let profilePath: String

	public init(workspacePath: String, profilePath: String) {
		self.workspacePath = workspacePath
		self.profilePath = profilePath
	}

	public init(_ workspace: Workspace) {
		self.init(workspacePath: workspace.path, profilePath: workspace.profilePath)
	}

	/// The worktree's directory name in the workspace: the repo's name, then `-<postfix>` when the postfix, trimmed, isn't empty, as Ember names it.
	public static func worktreeDirName(repo: String, postfix: String) -> String {
		let postfix = postfix.trimmingCharacters(in: .whitespacesAndNewlines)
		return postfix.isEmpty ? repo : "\(repo)-\(postfix)"
	}
}

/// One repo operation, from its start until it ends: its progress, then its outcome.
@MainActor
@Observable
public final class RepoOperationRun {
	public enum Outcome: Equatable, Sendable {
		/// Done: the workspace added to, or the clone (`path` and `name`).
		case done(CreatedWorkspace)
		/// Cancelled, once the core has undone the work.
		case cancelled
		/// Failed with this message, which also has its banner; the dialog shows it until closed.
		case failed(String)
		/// The core refused to start it; the failed call has its banner.
		case notStarted
	}

	public private(set) var progress = CreationProgress()
	/// Nil while it runs.
	public private(set) var outcome: Outcome?

	@ObservationIgnored private var id: UInt32?
	@ObservationIgnored private var cancelRequested = false
	@ObservationIgnored private let cancelOperation: @MainActor (UInt32) -> Void

	init(cancelOperation: @escaping @MainActor (UInt32) -> Void) {
		self.cancelOperation = cancelOperation
	}

	public var isRunning: Bool { outcome == nil }

	/// Asks the core to stop it; a cancel asked for before the core has returned its id is held until it has, and one after it has ended does nothing.
	public func cancel() {
		guard isRunning else { return }
		if let id { cancelOperation(id) } else { cancelRequested = true }
	}

	func started(id: UInt32) {
		self.id = id
		if cancelRequested { cancelOperation(id) }
	}

	func progressed(_ events: [CreationEvent]) {
		guard isRunning else { return }
		for event in events {
			progress.apply(event)
			switch event {
			case .done(let workspace): end(.done(workspace))
			case .failed(let message): end(.failed(message))
			case .cancelled: end(.cancelled)
			case .step, .log: break
			}
		}
	}

	func end(_ outcome: Outcome) {
		guard isRunning else { return }
		if case .failed(let message) = outcome { progress.fail(message) }
		self.outcome = outcome
	}
}
