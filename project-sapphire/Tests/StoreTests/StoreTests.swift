import CoreBindings
import Foundation
import Store
import Testing

/// A global config with its profiles and workspaces in a temporary directory, laid out as Devora creates them.
struct Fixture: ~Copyable {
	let root: URL
	var configPath: String { root.appending(path: "config.json").path }

	init() throws {
		root = FileManager.default.temporaryDirectory.appending(path: "sapphire-fixture-\(UUID().uuidString)")
		try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
	}

	deinit {
		try? FileManager.default.removeItem(at: root)
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

		let store = Store(configPath: fixture.configPath, state: .inMemory)
		await store.dispatch(.load).value

		#expect(store.error == nil)
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
		let store = Store(configPath: fixture.configPath, state: .inMemory)
		await store.dispatch(.load).value

		await store.dispatch(.selectProfile(path: home)).value

		#expect(store.selectedProfilePath == home)
		#expect(store.workspaces.map(\.title) == ["Home task"])
	}

	@Test func aCoreErrorSurfacesInState() async throws {
		let fixture = try Fixture()
		try fixture.writeConfig("{not json")

		let store = Store(configPath: fixture.configPath, state: .inMemory)
		await store.dispatch(.load).value

		let error = try #require(store.error)
		#expect(error.contains("failed to parse"))
		#expect(error.contains(fixture.configPath))
		#expect(store.profiles.isEmpty)
	}
}
