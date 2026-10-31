import CoreBindings
import Foundation
import Store
import Testing

@MainActor
struct ProfileTests {
	@Test func registeringANewProfileCreatesItAndSelectsIt() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		try fixture.registerProfiles([work])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value
		let root = fixture.root.appending(path: "personal").path

		#expect(await store.validateProfilePath(root) == .new(expandedPath: root))
		let registered = try #require(await store.registerProfile(path: root, name: "Personal"))
		await store.load(selecting: registered.path)

		#expect(FileManager.default.fileExists(atPath: "\(root)/workspaces"))
		#expect(store.profiles.map(\.name) == ["work", "Personal"])
		#expect(store.selectedProfilePath == root)
		#expect(store.scrapeErrors() == [])
	}

	@Test func aFailedRegistrationIsShownAndRegistersNothing() async throws {
		let fixture = try Fixture()
		try fixture.registerProfiles([])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value

		#expect(await store.registerProfile(path: "relative", name: "Work") == nil)

		#expect(store.scrapeErrors() == ["register_profile failed: Path must be absolute or start with ~"])
		await store.load()
		#expect(store.profiles.isEmpty)
	}

	@Test func unregisteringTheSelectedProfileSelectsTheFirstAndTheLastLeavesNone() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		let home = try fixture.makeProfile("home")
		try fixture.registerProfiles([work, home])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.load(selecting: home)

		#expect(await store.unregisterProfile(path: home))
		#expect(store.selectedProfilePath == work)

		#expect(await store.unregisterProfile(path: work))
		#expect(store.profilesListed && store.profiles.isEmpty)
		#expect(store.selectedProfilePath == nil)
		#expect(FileManager.default.fileExists(atPath: "\(work)/config.json"), "the directory stays on disk")
	}

	@Test func theTabsUsingAProfileAreItsWorkspaceTabs() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Task")
		try fixture.registerProfiles([work])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.dispatch(.load).value

		store.dispatch(.openShell)
		store.dispatch(.openWorkspace(store.workspaces[0]))
		try await eventually { store.app.window.tabs.count == 2 }

		#expect(store.tabs(usingProfile: work).map(\.title) == ["Task"])
	}

	@Test func aWorkspaceOpensWithItsOwnProfileWhenAnotherIsSelected() async throws {
		let fixture = try Fixture()
		let work = try fixture.makeProfile("work")
		let home = try fixture.makeProfile("home")
		try fixture.makeWorkspace(profile: work, id: "ws-1", title: "Task")
		try fixture.registerProfiles([work, home])
		let store = Store(configPath: fixture.configPath, state: .inMemory, log: try fixture.log())
		await store.load(selecting: work)
		let listed = store.workspaces[0]

		await store.dispatch(.selectProfile(path: home)).value
		store.dispatch(.openWorkspace(listed))
		try await eventually { store.app.window.tabs.count == 1 }

		#expect(store.app.window.tabs[0].session.profilePath == work)
	}
}
