import CoreBindings
import Foundation
import Store
import Testing

/// A global config with its profiles and workspaces in a temporary directory, laid out as Devora creates them.
struct Fixture: ~Copyable {
	let root: URL
	var configPath: String { root.appending(path: "config.json").path }
	var logPath: String { root.appending(path: "app.log").path }

	init() throws {
		root = FileManager.default.temporaryDirectory.appending(path: "sapphire-fixture-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
	}

	deinit {
		try? FileManager.default.removeItem(at: root)
	}

	/// A log at `logPath`, with a stream of reported errors of its own.
	func log() throws -> AppLog {
		try AppLog.open(path: logPath)
	}

	func logLines() throws -> [String] {
		try String(contentsOfFile: logPath, encoding: .utf8).split(separator: "\n").map(String.init)
	}

	func writeConfig(_ json: String) throws {
		try json.write(toFile: configPath, atomically: true, encoding: .utf8)
	}

	func registerProfiles(_ paths: [String]) throws {
		let data = try JSONSerialization.data(withJSONObject: ["profiles": paths])
		try writeConfig(String(decoding: data, as: UTF8.self))
	}

	func makeProfile(_ name: String) throws -> String {
		let profile = root.appending(path: name)
		for dir in ["workspaces", "repos"] {
			try FileManager.default.createDirectory(at: profile.appending(path: dir), withIntermediateDirectories: true)
		}
		try #"{"name": "\#(name)"}"#.write(to: profile.appending(path: "config.json"), atomically: true, encoding: .utf8)
		return profile.path
	}

	/// An inactive workspace has no `task.json`.
	func makeWorkspace(profile: String, id: String, title: String?) throws {
		let workspace = URL(filePath: profile).appending(path: "workspaces/\(id)")
		try FileManager.default.createDirectory(at: workspace, withIntermediateDirectories: true)
		try Data().write(to: workspace.appending(path: "initialized"))
		if let title {
			try #"{"uid": "\#(UUID())", "title": "\#(title)"}"#.write(to: workspace.appending(path: "task.json"), atomically: true, encoding: .utf8)
		}
	}
}

@MainActor
struct StoreTests {
	@Test func loadsProfilesAndTheFirstProfilesWorkspaces() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		let home = try fixture.makeProfile("home")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Fix the build")
		try fixture.makeWorkspace(profile: work, id: "ws-2", title: nil)
		try fixture.registerProfiles([work, home])

		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value

		#expect(store.scrapeErrors() == [])
		#expect(!store.version.isEmpty)
		#expect(store.profiles.map(\.name) == ["work", "home"])
		#expect(store.selectedProfilePath == work)
		#expect(store.workspaces.map(\.id) == ["ws-1", "ws-2"])
		#expect(store.workspaces.map(\.active) == [true, false])
		#expect(store.workspaces.first?.title == "Fix the build")
	}

	@Test func selectingAProfileShowsItsWorkspaces() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		let home = try fixture.makeProfile("home")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Work task")
		try fixture.makeWorkspace(profile: home, id: "ws-7", title: "Home task")
		try fixture.registerProfiles([work, home])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value

		await store.dispatch(.selectProfile(path: home)).value

		#expect(store.selectedProfilePath == home)
		#expect(store.workspaces.map(\.title) == ["Home task"])
	}

	@Test func loadsEachWorkspacesStatusAndOneThatCantBeReadIsInvalidWithoutABanner() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Readable")
		try fixture.makeWorkspace(profile: work, id: "ws-2", title: "Unreadable")
		let api = "\(work)/workspaces/ws-1/api"
		let web = "\(work)/workspaces/ws-2/web"
		for repo in [api, web] {
			try FileManager.default.createDirectory(atPath: repo, withIntermediateDirectories: true)
		}
		let initialize = try Process.run(URL(filePath: "/usr/bin/git"), arguments: ["init", "-q", api])
		initialize.waitUntilExit()
		try Data().write(to: URL(filePath: "\(api)/new.txt"))
		try fixture.registerProfiles([work])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value
		// Gone between the listing and the status, as when a repo is removed under the hub.
		try FileManager.default.removeItem(atPath: web)

		store.dispatch(.loadWorkspaceStatuses(fresh: false))
		try await eventually { store.workspaceStatuses.count == 2 }

		let readable = try #require(store.workspaceStatuses["\(work)/workspaces/ws-1"])
		#expect(readable.error == nil)
		#expect(readable.repos.map { "\($0.name) +\($0.untracked)" } == ["api +1"])
		let unreadable = try #require(store.workspaceStatuses["\(work)/workspaces/ws-2"])
		#expect(unreadable.error?.contains("web") == true)
		#expect(store.scrapeErrors() == [])

		store.dispatch(.loadWorkspaceStatuses(fresh: true))
		#expect(store.workspaceStatuses.isEmpty, "a fresh load drops the statuses it is replacing")
		try await eventually { store.workspaceStatuses.count == 2 }
	}

	@Test func removingATaskOrDeletingAWorkspaceListsTheWorkspacesAgain() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Done")
		try fixture.makeWorkspace(profile: work, id: "ws-2", title: nil)
		try fixture.registerProfiles([work])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value

		#expect(await store.removeTask(store.workspaces[0]))
		#expect(store.workspaces.map(\.active) == [false, false])
		#expect(await store.deleteWorkspace(store.workspaces[1]))
		#expect(store.workspaces.map(\.id) == ["ws-1"])
		#expect(store.scrapeErrors() == [])
	}

	@Test func theLoadingLatenciesReportTheListingAndEachStatus() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "One")
		try fixture.makeWorkspace(profile: work, id: "ws-2", title: "Two")
		try fixture.registerProfiles([work])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value
		store.dispatch(.loadWorkspaceStatuses(fresh: true))
		try await eventually { store.workspaceStatuses.count == 2 }

		let path = try #require(await store.saveLoadingLatencies())

		#expect(path.hasPrefix("\(work)/diagnostics/hub-profile-") && path.hasSuffix(".json"))
		let report = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: URL(filePath: path))) as? [String: Any])
		#expect(report["profileName"] as? String == "work")
		#expect(report["workspaceCount"] as? Int == 2)
		let phases = try #require(report["phases"] as? [String: [String: Double]])
		#expect(Set(phases.keys) == ["listProfiles", "listWorkspaces", "totalLoad"])
		#expect(phases["totalLoad"]!["durationMs"]! >= phases["listWorkspaces"]!["durationMs"]!)
		let statuses = try #require(report["workspaceStatuses"] as? [[String: Any]])
		#expect(Set(statuses.compactMap { $0["wsId"] as? String }) == ["ws-1", "ws-2"])
	}

	@Test func aFailingCoreCallIsShownOnABannerAndLogged() async throws {
		let fixture = try Fixture()
		try fixture.writeConfig("{not json")

		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value

		let banner = try #require(store.errorBanners.visible.only)
		#expect(banner.message.hasPrefix("list_profiles failed: "))
		#expect(banner.message.contains("failed to parse"))
		#expect(banner.message.contains(fixture.configPath))
		#expect(store.scrapeErrors() == [banner.message])
		#expect(try fixture.logLines().map { $0.hasSuffix("[ERROR] \(banner.message)") } == [true])
		#expect(store.profiles.isEmpty)
	}

	@Test func theSameFailureAgainCountsOnItsBannerUntilItIsDismissed() async throws {
		let fixture = try Fixture()
		try fixture.writeConfig("{not json")
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())

		await store.dispatch(.load).value
		await store.dispatch(.load).value

		let banner = try #require(store.errorBanners.visible.only)
		#expect(banner.count == 2)
		#expect(store.scrapeErrors().count == 2, "each one is recorded")

		store.dispatch(.dismissError(id: banner.id))
		#expect(store.errorBanners.visible.isEmpty)
		await store.dispatch(.load).value
		#expect(store.errorBanners.visible.map(\.count) == [1], "a dismissed message starts a new banner")
	}

	@Test func anErrorTheCoreReportsReachesABannerAndIsLoggedOnce() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Fine")
		try fixture.makeWorkspace(profile: work, id: "ws-2", title: nil)
		try "{corrupt".write(toFile: "\(work)/workspaces/ws-2/task.json", atomically: true, encoding: .utf8)
		try fixture.registerProfiles([work])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())

		await store.dispatch(.load).value
		try await eventually { !store.errorBanners.visible.isEmpty }

		let banner = try #require(store.errorBanners.visible.only)
		#expect(banner.message.contains("failed to parse"))
		#expect(store.workspaces.map(\.id) == ["ws-1", "ws-2"], "the listing still lists it")
		#expect(store.scrapeErrors() == [banner.message])
		#expect(try fixture.logLines().filter { $0.contains("[ERROR]") }.count == 1)
	}
}

@Suite struct ErrorBannersTests {
	@Test func anIdenticalMessageCountsOnItsBannerAndOthersStackBelow() {
		var banners = ErrorBanners()

		banners.show("a failed")
		banners.show("b failed")
		banners.show("a failed")

		#expect(banners.visible.map(\.message) == ["a failed", "b failed"])
		#expect(banners.visible.map(\.count) == [2, 1])
	}

	@Test func dismissingRemovesOnlyThatBannerAndItsMessageStartsOver() {
		var banners = ErrorBanners()
		banners.show("a failed")
		banners.show("a failed")
		banners.show("b failed")
		let a = banners.visible[0]

		banners.dismiss(id: a.id)
		banners.show("a failed")

		#expect(banners.visible.map(\.message) == ["b failed", "a failed"])
		#expect(banners.visible.map(\.count) == [1, 1])
		#expect(banners.visible[1].id != a.id)
	}
}

extension Collection {
	/// The one element, or nil when there are none or several.
	var only: Element? { count == 1 ? first : nil }
}
