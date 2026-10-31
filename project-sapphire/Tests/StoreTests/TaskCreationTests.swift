import CoreBindings
import Foundation
import Store
import Testing

extension Fixture {
	/// A selected profile `work` whose one repo, `repo`, has a bare origin with a commit on `main`, as a creation needs; `prepare` is its prepare-command.
	func originBackedProfile(prepare: String?) throws -> (profile: String, repo: String) {
		let profile = try makeProfile("work")
		if let prepare {
			let config = try JSONSerialization.data(withJSONObject: ["name": "work", "prepare-command": prepare])
			try config.write(to: URL(filePath: "\(profile)/config.json"))
		}
		let origin = root.appending(path: "repo.git").path
		let repo = "\(profile)/repos/repo"
		try FileManager.default.createDirectory(atPath: origin, withIntermediateDirectories: true)
		try FileManager.default.createDirectory(atPath: repo, withIntermediateDirectories: true)
		try git(["init", "-q", "--bare", "-b", "main"], in: origin)
		try git(["init", "-q", "-b", "main"], in: repo)
		try "v1\n".write(toFile: "\(repo)/file.txt", atomically: true, encoding: .utf8)
		try git(["add", "."], in: repo)
		try git(["commit", "-q", "-m", "c1"], in: repo)
		try git(["remote", "add", "origin", origin], in: repo)
		try git(["push", "-q", "-u", "origin", "main"], in: repo)
		try registerProfiles([profile])
		return (profile, repo)
	}

	private func git(_ arguments: [String], in directory: String) throws {
		let process = Process()
		process.executableURL = URL(filePath: "/usr/bin/git")
		process.arguments = ["-c", "user.name=t", "-c", "user.email=t@t"] + arguments
		process.currentDirectoryURL = URL(filePath: directory)
		try process.run()
		process.waitUntilExit()
		guard process.terminationStatus == 0 else { throw CocoaError(.fileWriteUnknown) }
	}
}

/// Task creation through the store, against the real core and real git repos.
@MainActor
struct TaskCreationTests {
	private func loadedStore(_ fixture: borrowing Fixture) async throws -> Store {
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value
		return store
	}

	private func onlyTab(of store: Store) throws -> AppTab {
		try #require(store.app.window.tabs.only)
	}

	@Test func aTaskOpensACreatingTabThatGetsItsWorkspace() async throws {
		let fixture = try Fixture()
		let (profile, repo) = try fixture.originBackedProfile(prepare: nil)
		let store = try await loadedStore(fixture)

		await store.dispatch(.createTask(title: "Fix login", repoPaths: [repo], sourceWorkspacePath: nil)).value
		try await eventually { store.app.window.tabs.count == 1 }
		let tab = try onlyTab(of: store)
		#expect(tab.title == "Fix login")
		#expect(store.app.window.activeTab == tab.id)

		try await eventually { store.app.window.tabs.first?.session.kind != .creatingWorkspace }
		#expect(try onlyTab(of: store).session.kind == .workspace(path: "\(profile)/workspaces/ws-1"))
		#expect(store.creations.isEmpty, "the progress goes once the tab has its workspace")
		#expect(store.scrapeErrors() == [])
	}

	@Test func theProgressFollowsTheStepsAndACancelUndoesTheWork() async throws {
		let fixture = try Fixture()
		let (profile, repo) = try fixture.originBackedProfile(prepare: "echo preparing; sleep 30")
		let store = try await loadedStore(fixture)

		await store.dispatch(.createTask(title: "Will cancel", repoPaths: [repo], sourceWorkspacePath: nil)).value
		try await eventually { store.creations.values.first?.output.contains("preparing") == true }
		let tab = try onlyTab(of: store)
		let progress = try #require(store.creations[tab.id])
		#expect(progress.steps.first == .init(label: "Resolving workspace", state: .done))
		#expect(progress.steps.last == .init(label: "Preparing repo", state: .active))

		store.dispatch(.cancelCreation(tab: tab.id))

		try await eventually { store.app.window.tabs.isEmpty }
		#expect(store.creations.isEmpty)
		#expect(!FileManager.default.fileExists(atPath: "\(profile)/workspaces/ws-1"))
		#expect(store.scrapeErrors() == [])
	}

	@Test func aCancelAskedForBeforeTheCreationStartedIsAppliedOnceItHas() async throws {
		let fixture = try Fixture()
		let (profile, repo) = try fixture.originBackedProfile(prepare: "sleep 30")
		let store = try await loadedStore(fixture)
		store.dispatch(.openShell)
		try await eventually { store.app.window.tabs.count == 1 }
		// The core numbers tabs and panes from one counter, so the task's tab comes right after the shell's pane.
		let shell = try onlyTab(of: store)
		let taskTab = max(shell.id, shell.terminalID) + 1

		store.dispatch(.cancelCreation(tab: taskTab))
		await store.dispatch(.createTask(title: "Too late", repoPaths: [repo], sourceWorkspacePath: nil)).value

		try await eventually { store.app.window.tabs.count == 2 }
		#expect(store.app.window.tabs.last?.id == taskTab)
		try await eventually { store.app.window.tabs.count == 1 }
		#expect(!FileManager.default.fileExists(atPath: "\(profile)/workspaces/ws-1"))
		#expect(store.scrapeErrors() == [])
	}

	@Test func aFailureStaysOnItsTabWithItsBanner() async throws {
		let fixture = try Fixture()
		let (profile, _) = try fixture.originBackedProfile(prepare: nil)
		let store = try await loadedStore(fixture)

		await store.dispatch(.createTask(title: "Doomed", repoPaths: ["\(profile)/repos/missing"], sourceWorkspacePath: nil)).value

		try await eventually {
			guard case .creationFailed? = store.app.window.tabs.first?.session.kind else { return false }
			return store.creations.values.first?.failure != nil && !store.errorBanners.visible.isEmpty
		}
		guard case .creationFailed(let reason) = try onlyTab(of: store).session.kind else { return }
		let progress = try #require(store.creations.values.first)
		#expect(progress.failure == reason)
		#expect(store.scrapeErrors() == [reason])

		store.dispatch(.closeTab(id: try onlyTab(of: store).id))
		try await eventually { store.app.window.tabs.isEmpty }
		#expect(store.creations.isEmpty)
	}

	@Test func closingTheTabOfARunningCreationCancelsIt() async throws {
		let fixture = try Fixture()
		let (profile, repo) = try fixture.originBackedProfile(prepare: "echo preparing; sleep 30")
		let store = try await loadedStore(fixture)
		await store.dispatch(.createTask(title: "Abandoned", repoPaths: [repo], sourceWorkspacePath: nil)).value
		try await eventually { store.creations.values.first?.output.contains("preparing") == true }

		store.dispatch(.closeTab(id: try onlyTab(of: store).id))

		try await eventually { !FileManager.default.fileExists(atPath: "\(profile)/workspaces/ws-1") }
		#expect(store.creations.isEmpty)
		#expect(store.scrapeErrors() == [])
	}

	@Test func theRegisteredReposAreTheSelectedProfiles() async throws {
		let fixture = try Fixture()
		let (_, repo) = try fixture.originBackedProfile(prepare: nil)
		let store = try await loadedStore(fixture)

		let repos = try #require(await store.registeredRepos())

		#expect(repos.map(\.name) == ["repo"])
		#expect(repos.map(\.path) == [repo])
	}
}

/// The progress page's fold of the creation events, as Ember's `TaskCreationProgress` shows them.
struct CreationProgressTests {
	@Test func eachStepFinishesTheOneBeforeAndDoneFinishesTheLast() {
		var progress = CreationProgress()

		progress.apply(.step(label: "Resolving workspace"))
		progress.apply(.step(label: "Fetching repo"))
		#expect(progress.steps.map(\.state) == [.done, .active])

		progress.apply(.done(workspace: CreatedWorkspace(path: "/ws-1", name: "ws-1")))
		#expect(progress.steps.map(\.state) == [.done, .done])
		#expect(progress.failure == nil)
	}

	@Test func aFailureFailsTheActiveStepAndTheFirstOneIsKept() {
		var progress = CreationProgress()
		progress.apply(.step(label: "Preparing repo"))

		progress.apply(.failed(message: "prepare-command failed"))
		progress.fail("interrupted")

		#expect(progress.steps.map(\.state) == [.failed])
		#expect(progress.failure == "prepare-command failed", "the first failure is the one shown")
	}

	@Test func theOutputKeepsItsLatestLines() {
		var progress = CreationProgress()

		for i in 1...(CreationProgress.outputLimit + 2) { progress.apply(.log(line: "line \(i)")) }

		#expect(progress.output.count == CreationProgress.outputLimit)
		#expect(progress.output.first == "line 3")
		#expect(progress.output.last == "line \(CreationProgress.outputLimit + 2)")
	}
}
