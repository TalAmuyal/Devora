import CoreBindings
import Foundation
import Store
import Testing

/// Add Repo and Clone Repo through the store, against the real core and real git repos.
@MainActor
struct RepoOperationTests {
	/// A store on the origin-backed profile, with a task whose workspace, `ws-1`, has the profile's repo; its tab is the active one.
	private func storeWithATask(_ fixture: borrowing Fixture, prepare: String?) async throws -> (Store, profile: String, repo: String) {
		let (profile, repo) = try fixture.originBackedProfile(prepare: prepare)
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value
		await store.dispatch(.createTask(title: "Main work", repoPaths: [repo], sourceWorkspacePath: nil)).value
		try await eventually(timeout: .seconds(30)) {
			guard case .workspace? = store.app.window.tabs.first?.session.kind else { return false }
			return true
		}
		return (store, profile, repo)
	}

	/// Starts the operation and waits for the store to have followed it to its end.
	private func run(_ request: RepoOperationRequest, on store: Store) async -> (RepoOperationRun, RepoOperationRun.Outcome) {
		var run: RepoOperationRun!
		let outcome = await withCheckedContinuation { continuation in
			run = store.startRepoOperation(request) { continuation.resume(returning: $0) }
		}
		return (run, outcome)
	}

	@Test func addingARepoToTheActiveTabsWorkspaceStreamsItsStepsThenListsTheWorkspaceWithIt() async throws {
		let fixture = try Fixture()
		let (store, profile, repo) = try await storeWithATask(fixture, prepare: nil)
		let workspace = "\(profile)/workspaces/ws-1"
		let target = try #require(store.addRepoTargetOfActiveTab())
		#expect(target == AddRepoTarget(workspacePath: workspace, profilePath: profile))

		let dirName = AddRepoTarget.worktreeDirName(repo: "repo", postfix: " ref ")
		let (run, outcome) = await run(
			.addRepo(workspacePath: target.workspacePath, sourceRepoPath: repo, worktreeDirName: dirName), on: store)

		#expect(outcome == .done(CreatedWorkspace(path: workspace, name: "ws-1")))
		// `mise trust` is a step only where mise is installed, and the profile has no prepare-command.
		#expect(run.progress.steps.map(\.label).filter { !$0.hasPrefix("Trusting mise") } == ["Fetching repo-ref", "Creating worktree repo-ref"])
		#expect(run.progress.steps.allSatisfy { $0.state == .done })
		#expect(FileManager.default.fileExists(atPath: "\(workspace)/repo-ref/file.txt"))
		#expect(store.workspaces.first { $0.path == workspace }?.repos.sorted() == ["repo", "repo-ref"], "the store lists the workspaces again before the end")
		#expect(store.scrapeErrors() == [])
	}

	@Test func aCancelWhilePreparingRemovesTheAddedWorktree() async throws {
		let fixture = try Fixture()
		let (store, profile, repo) = try await storeWithATask(fixture, prepare: nil)
		let workspace = "\(profile)/workspaces/ws-1"
		// The task is created; the add's own prepare-command is the one that sleeps.
		let config = try JSONSerialization.data(withJSONObject: ["name": "work", "prepare-command": "echo preparing; sleep 30"])
		try config.write(to: URL(filePath: "\(profile)/config.json"))

		var outcome: RepoOperationRun.Outcome?
		let run = store.startRepoOperation(.addRepo(workspacePath: workspace, sourceRepoPath: repo, worktreeDirName: "repo-ref")) {
			outcome = $0
		}
		try await eventually(timeout: .seconds(30)) { run.progress.output.contains("preparing") }
		run.cancel()

		try await eventually(timeout: .seconds(20)) { outcome != nil }
		#expect(outcome == .cancelled)
		#expect(!FileManager.default.fileExists(atPath: "\(workspace)/repo-ref"))
		#expect(FileManager.default.fileExists(atPath: "\(workspace)/repo/file.txt"))
		#expect(store.scrapeErrors() == [])
	}

	@Test func aNameTheWorkspaceHasFailsWithItsBannerAndTheRunKeepsTheFailure() async throws {
		let fixture = try Fixture()
		let (store, profile, repo) = try await storeWithATask(fixture, prepare: nil)

		let (run, outcome) = await run(
			.addRepo(workspacePath: "\(profile)/workspaces/ws-1", sourceRepoPath: repo, worktreeDirName: "repo"), on: store)

		let message = "Directory 'repo' already exists. Use a different postfix."
		#expect(outcome == .failed(message))
		#expect(run.progress.failure == message)
		try await eventually { store.errorBanners.visible.map(\.message) == [message] }
		#expect(store.scrapeErrors() == [message])
	}

	@Test func aShellTabHasNoWorkspaceToAddTo() async throws {
		let fixture = try Fixture()
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		store.dispatch(.openShell)
		try await eventually { store.app.window.tabs.count == 1 }

		#expect(store.addRepoTargetOfActiveTab() == nil)
		#expect(store.scrapeErrors() == ["Cannot add a repo: the current session has no associated workspace"])
	}

	@Test func cloningPutsTheRepoDetachedWhereTheHintSaid() async throws {
		let fixture = try Fixture()
		let (profile, _) = try fixture.originBackedProfile(prepare: nil)
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		let other = fixture.root.appending(path: "other.git").path
		try FileManager.default.copyItem(atPath: fixture.root.appending(path: "repo.git").path, toPath: other)
		let url = "file://\(other)"
		let destination = "\(profile)/repos/other"
		#expect(store.cloneDestination(profilePath: profile, url: url) == destination)
		#expect(store.cloneDestination(profilePath: profile, url: "not a url") == nil)

		let (run, outcome) = await run(.cloneRepo(profilePath: profile, url: url), on: store)

		#expect(outcome == .done(CreatedWorkspace(path: destination, name: "other")))
		#expect(run.progress.steps.map(\.label) == ["Cloning other", "Checking out detached"])
		#expect(FileManager.default.fileExists(atPath: "\(destination)/file.txt"))
		#expect(await store.registeredRepos(inProfile: profile)?.map(\.path).contains(destination) == true)
		#expect(store.scrapeErrors() == [])
	}
}
