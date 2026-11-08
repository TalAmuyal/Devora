import CoreBindings

extension Store {
	/// Starts the app-owned scheduler after launch. Operational failures stay in the log.
	public func startRepoUpdates() async {
		guard let core else { return }
		do { try await Task.detached { try core.startRepoUpdates() }.value }
		catch { logWarning("start_repo_updates failed: \(error)") }
	}

	public func runRepoUpdates() async {
		guard let core else { return }
		do { try await Task.detached { try core.runRepoUpdates() }.value }
		catch { logWarning("run_repo_updates failed: \(error)") }
	}
	public func repoUpdatesWoke() async {
		guard let core else { return }
		do { try await Task.detached { try core.repoUpdatesWoke() }.value }
		catch { logWarning("repo_updates_woke failed: \(error)") }
	}

}
