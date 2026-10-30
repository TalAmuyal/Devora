import CoreBindings
import Foundation
import Observation

public enum Command: Sendable {
	/// Loads the version and the profiles, then the workspaces of the selected (or first) profile.
	case load
	case selectProfile(path: String)
}

/// The single app store: views read its state and dispatch `Command`s; only the store calls the core.
@MainActor
@Observable
public final class Store {
	public private(set) var version = ""
	public private(set) var profiles: [Profile] = []
	public private(set) var selectedProfilePath: String?
	public private(set) var workspaces: [Workspace] = []
	/// Problems the core reported without failing, such as a corrupt `task.json`.
	public private(set) var warnings: [String] = []
	/// The last command's failure, cleared by the next success.
	public private(set) var error: String?

	@ObservationIgnored private let core: DevoraCore?

	/// `configPath` overrides the global config file (by default `DEVORA_CONFIG_PATH`, then `~/.config/devora/config.json`).
	public init(configPath: String? = nil) {
		do {
			core = try DevoraCore(configPath: configPath)
		} catch {
			core = nil
			self.error = Self.describe(error)
		}
	}

	@discardableResult
	public func dispatch(_ command: Command) -> Task<Void, Never> {
		Task { await perform(command) }
	}

	private func perform(_ command: Command) async {
		guard let core else { return }
		switch command {
		case .load:
			do {
				let (version, profiles) = try await Self.offMain { (try core.version(), try core.listProfiles()) }
				self.version = version
				self.profiles = profiles
				error = nil
				let selected = profiles.first { $0.path == selectedProfilePath } ?? profiles.first
				await select(selected?.path, core: core)
			} catch {
				self.error = Self.describe(error)
			}
		case .selectProfile(let path):
			await select(path, core: core)
		}
	}

	private func select(_ path: String?, core: DevoraCore) async {
		selectedProfilePath = path
		guard let path else {
			workspaces = []
			warnings = []
			return
		}
		do {
			let list = try await Self.offMain { try core.listWorkspaces(profilePath: path) }
			// A later selection wins over a slower, earlier one.
			guard selectedProfilePath == path else { return }
			workspaces = list.workspaces
			warnings = list.warnings
			error = nil
		} catch {
			guard selectedProfilePath == path else { return }
			workspaces = []
			warnings = []
			self.error = Self.describe(error)
		}
	}

	/// Core calls do file I/O, so they run off the main actor.
	private static func offMain<T: Sendable>(_ body: @escaping @Sendable () throws -> T) async throws -> T {
		try await Task.detached(operation: body).value
	}

	private static func describe(_ error: any Error) -> String {
		if case .Failed(let message) = error as? CoreError { return message }
		return String(describing: error)
	}
}
