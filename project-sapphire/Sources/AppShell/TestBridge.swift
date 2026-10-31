import AppKit
import CoreBindings
import Foundation
import Layers
import Network
import SwiftUI
import Store
import TerminalKit

/// The acceptance tests' way into the app, as Ember's test harness is: a loopback HTTP server, started only when `DEVORA_TEST_MODE=1`.
///
/// It dispatches the store's commands and reads back the store's state and the terminals' text; the routes are listed in `CLAUDE.md`.
@MainActor
final class TestBridge {
	static var isEnabled: Bool { ProcessInfo.processInfo.environment["DEVORA_TEST_MODE"] == "1" }

	/// Keeps the bridge alive for the rest of the process.
	private static var running: TestBridge?

	private let store: Store
	private let terminals: Terminals
	private let surfaces: Surfaces
	private let listener: NWListener

	/// Starts the bridge, unless it is running, and prints its port to stderr, where the harness reads it, as Ember does.
	static func start(store: Store, terminals: Terminals, surfaces: Surfaces) {
		guard running == nil else { return }
		do {
			let bridge = try TestBridge(store: store, terminals: terminals, surfaces: surfaces)
			running = bridge
			bridge.listen()
		} catch {
			NSLog("test bridge: failed to start: %@", String(describing: error))
		}
	}

	private init(store: Store, terminals: Terminals, surfaces: Surfaces) throws {
		self.store = store
		self.terminals = terminals
		self.surfaces = surfaces
		let parameters = NWParameters.tcp
		parameters.requiredLocalEndpoint = .hostPort(host: .ipv4(.loopback), port: .any)
		listener = try NWListener(using: parameters)
	}

	private func listen() {
		listener.stateUpdateHandler = { [listener] state in
			switch state {
			case .ready:
				if let port = listener.port?.rawValue {
					FileHandle.standardError.write(Data("Devora test harness on port \(port)\n".utf8))
				}
			case .failed(let error):
				NSLog("test bridge: %@", String(describing: error))
			default:
				break
			}
		}
		listener.newConnectionHandler = { [self] connection in
			connection.start(queue: .main)
			HTTPExchange(connection: connection).receive { request in await self.respond(to: request) }
		}
		listener.start(queue: .main)
	}

	private func respond(to request: HTTPRequest) async -> HTTPResponse {
		do {
			switch (request.method, request.path) {
			case ("GET", "/test/state"):
				return .init(status: 200, body: state())
			case ("POST", "/test/dispatch"):
				try await dispatch(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/quit"):
				// The app may exit before the response is out, so the harness waits for the exit instead.
				Task { NSApp.terminate(nil) }
				return .init(status: 200, body: [:])
			case ("GET", "/test/terminal/text"):
				let session = try terminal(request.query["tab"].flatMap { UInt64($0) })
				return .init(status: 200, body: ["text": session.allText() ?? ""])
			case ("POST", "/test/terminal/type"):
				let body = try request.json()
				let session = try terminal((body["tab"] as? NSNumber)?.uint64Value)
				guard let line = body["line"] as? String else { throw BridgeError(status: 400, "missing line") }
				session.typeLine(line)
				return .init(status: 200, body: [:])
			case ("POST", "/test/key"):
				return .init(status: 200, body: ["consumed": try press(request.json())])
			case ("POST", "/test/hub"):
				try hubAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/dialog"):
				try dialogButton(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/new-task"):
				try newTaskAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/profile-form"):
				try profileFormAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/layers/clear"):
				// Teardown, as Ember's `__test.layers.clear()`: it closes even the first-run welcome, which refuses `q` and Escape.
				surfaces.windowStack.clear()
				surfaces.router.focusActiveTab()
				return .init(status: 200, body: [:])
			case ("POST", "/test/progress"):
				try progressAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/terminal/focus"):
				let body = try request.json()
				try terminal((body["tab"] as? NSNumber)?.uint64Value).focus()
				return .init(status: 200, body: [:])
			case ("POST", "/test/snapshot"):
				guard let path = try request.json()["path"] as? String else { throw BridgeError(status: 400, "missing path") }
				await Snapshot.captureWindow(to: path, store: store)
				return .init(status: 200, body: [:])
			case ("POST", "/test/errors/scrape"):
				return .init(status: 200, body: ["errors": store.scrapeErrors()])
			default:
				return .init(status: 404, body: ["error": "no route \(request.method) \(request.path)"])
			}
		} catch let error as BridgeError {
			return .init(status: error.status, body: ["error": error.message])
		} catch {
			return .init(status: 500, body: ["error": String(describing: error)])
		}
	}

	private func dispatch(_ body: [String: Any]) async throws {
		let command: Command
		switch body["command"] as? String {
		case "load": command = .load
		case "openShell": command = .openShell
		case "selectProfile":
			guard let path = body["path"] as? String else { throw BridgeError(status: 400, "missing path") }
			command = .selectProfile(path: path)
		case "openWorkspace":
			let id = body["id"] as? String
			guard let workspace = store.workspaces.first(where: { $0.id == id }) else {
				throw BridgeError(status: 404, "no workspace \(id ?? "nil") in the selected profile")
			}
			command = .openWorkspace(workspace)
		case "activateTab": command = .activateTab(id: try uint64(body, "id"))
		case "closeTab": command = .closeTab(id: try uint64(body, "id"))
		case "startFresh": command = .startFresh
		case "dismissError": command = .dismissError(id: try uint64(body, "id"))
		default:
			throw BridgeError(status: 400, "unknown command \(String(describing: body["command"]))")
		}
		await store.dispatch(command).value
	}

	private func uint64(_ body: [String: Any], _ key: String) throws -> UInt64 {
		guard let value = (body[key] as? NSNumber)?.uint64Value else { throw BridgeError(status: 400, "missing \(key)") }
		return value
	}

	/// The terminal of the tab with this id (the active tab by default); a 409 until the tab has one, so that callers poll.
	private func terminal(_ id: UInt64?) throws -> TerminalSession {
		let window = store.app.window
		guard let id = id ?? window.activeTab, let tab = window.tabs.first(where: { $0.id == id }) else {
			throw BridgeError(status: 404, "no tab \(id.map(String.init) ?? "is active")")
		}
		guard let session = terminals.session(for: tab) else { throw BridgeError(status: 409, "tab \(id) has no terminal yet") }
		return session
	}

	/// Presses a key in the window: a key-down event enters the router as AppKit's would, and, unless it is consumed, goes on to the window, which dispatches it to its first responder; returns whether the router consumed it.
	private func press(_ body: [String: Any]) throws -> Bool {
		guard let name = body["key"] as? String, let key = TestKey(name) else {
			throw BridgeError(status: 400, "unknown key \(String(describing: body["key"]))")
		}
		var modifiers: KeyModifiers = []
		for modifier in body["modifiers"] as? [String] ?? [] {
			switch modifier {
			case "shift": modifiers.insert(.shift)
			case "control": modifiers.insert(.control)
			case "option": modifiers.insert(.option)
			case "command": modifiers.insert(.command)
			default: throw BridgeError(status: 400, "unknown modifier \(modifier)")
			}
		}
		guard let window = NSApp.windows.first(where: { $0.isVisible && $0.contentView != nil }) else {
			throw BridgeError(status: 409, "no window")
		}
		let event = try key.event(modifiers: modifiers, window: window)
		let consumed = surfaces.router.handle(KeyPress(event))
		if !consumed { window.sendEvent(event) }
		return consumed
	}

	/// Clicks what the open hub draws, as its buttons do: an action on the focused workspace, a dropdown's trigger, or a row of the open dropdown (by its label; a disabled one is a 409).
	private func hubAction(_ body: [String: Any]) throws {
		guard let hub = surfaces.hub else { throw BridgeError(status: 409, "the Workspace Hub isn't open") }
		switch body["action"] as? String {
		case "removeTask", "delete":
			guard let workspace = hub.focused else { throw BridgeError(status: 409, "no workspace is focused") }
			let removes = body["action"] as? String == "removeTask"
			guard workspace.active == removes else {
				throw BridgeError(status: 409, "\(workspace.id) has no \(removes ? "Remove Task" : "Delete") button")
			}
			if removes { hub.removeTask(workspace) } else { hub.deleteWorkspace(workspace) }
		case "newTask":
			hub.newTask(nil)
		case "duplicate":
			guard let workspace = hub.focused, workspace.active else { throw BridgeError(status: 409, "no active workspace is focused") }
			hub.newTask(workspace)
		case "toggleMenu":
			guard let menu = (body["menu"] as? String).flatMap(HubMenu.init) else { throw BridgeError(status: 400, "unknown menu") }
			hub.toggleMenu(menu)
		case "menuItem":
			guard let menu = hub.openMenu else { throw BridgeError(status: 409, "no dropdown is open") }
			let label = body["label"] as? String
			guard let item = hub.menuItems(menu).first(where: { $0.kind != .separator && $0.label == label }) else {
				throw BridgeError(status: 404, "no row \(label ?? "nil") in the \(menu.rawValue) dropdown")
			}
			guard item.enabled else { throw BridgeError(status: 409, "\(item.label) is disabled") }
			hub.closeMenu()
			item.select()
		default:
			throw BridgeError(status: 400, "unknown hub action \(String(describing: body["action"]))")
		}
	}

	/// Clicks one of the open confirmation dialog's buttons.
	private func dialogButton(_ body: [String: Any]) throws {
		guard let dialog = surfaces.dialog else { throw BridgeError(status: 409, "no confirmation dialog is open") }
		switch body["button"] as? String {
		case "confirm": dialog.confirm()
		case "cancel": dialog.cancel()
		default: throw BridgeError(status: 400, "unknown button \(String(describing: body["button"]))")
		}
	}

	/// Clicks what the open New Task dialog draws: a repo's row (by its name), or one of its buttons.
	private func newTaskAction(_ body: [String: Any]) throws {
		guard let dialog = surfaces.newTask else { throw BridgeError(status: 409, "the New Task dialog isn't open") }
		switch body["action"] as? String {
		case "toggleRepo":
			let name = body["name"] as? String
			guard let repo = dialog.visibleRepos.first(where: { $0.name == name }) else {
				throw BridgeError(status: 404, "no repo \(name ?? "nil") in the New Task dialog's list")
			}
			dialog.click(repo)
		case "create": dialog.create()
		case "cancel": dialog.cancel()
		default: throw BridgeError(status: 400, "unknown New Task action \(String(describing: body["action"]))")
		}
	}

	/// The open profile form: the New Profile dialog's, or else the first-run welcome's.
	private var profileForm: ProfileFormModel? {
		surfaces.newProfile ?? surfaces.hub?.welcome
	}

	/// Fills in the open profile form's fields, as typing into them does (`setName`, `setPath` with a `value`), or clicks its buttons (`submit`, `cancel`).
	private func profileFormAction(_ body: [String: Any]) throws {
		guard let form = profileForm else { throw BridgeError(status: 409, "no profile form is open") }
		let value = body["value"] as? String
		switch body["action"] as? String {
		case "setName":
			guard let value else { throw BridgeError(status: 400, "missing value") }
			form.typedName = value
		case "setPath":
			guard let value else { throw BridgeError(status: 400, "missing value") }
			form.editPath(value)
		case "submit":
			guard form.canSubmit else { throw BridgeError(status: 409, "the submit button is disabled") }
			form.submit()
		case "cancel":
			guard form.hasCancel else { throw BridgeError(status: 409, "the form has no Cancel button") }
			form.cancel()
		default:
			throw BridgeError(status: 400, "unknown profile form action \(String(describing: body["action"]))")
		}
	}

	private func profileFormState() -> Any {
		guard let form = profileForm else { return NSNull() }
		let status: Any = switch form.status {
		case .new: ["kind": "new"]
		case .existing(let name): ["kind": "existing", "name": name]
		case .invalid(let error): ["kind": "invalid", "error": error]
		case nil: NSNull()
		}
		return [
			"name": form.name,
			"nameLocked": form.isNameLocked,
			"path": form.path,
			"status": status,
			"validating": form.validating,
			"submitLabel": form.submitLabel,
			"canSubmit": form.canSubmit,
			"focus": form.focus.map { "\($0)" } ?? NSNull(),
		] as [String: Any]
	}

	/// Clicks the button of a tab's progress page (the active tab's by default): Cancel while its creation runs, Close once it has failed.
	private func progressAction(_ body: [String: Any]) throws {
		guard let id = (body["tab"] as? NSNumber)?.uint64Value ?? store.app.window.activeTab,
		      surfaces.tabStacks[id]?.find(CreationProgressPage.layerName) != nil
		else { throw BridgeError(status: 409, "the tab has no progress page") }
		CreationPageState(store: store, tab: id).closeOrCancel()
	}

	private func newTaskState() -> Any {
		guard let dialog = surfaces.newTask else { return NSNull() }
		let active = dialog.activeRepo
		return [
			"heading": dialog.heading,
			"title": dialog.title,
			"filter": dialog.filter,
			"focus": "\(dialog.focus)",
			"repos": dialog.visibleRepos.map { repo in
				["name": repo.name, "selected": dialog.selected.contains(repo.path), "active": repo == active]
			},
		] as [String: Any]
	}

	/// A tab's progress page, while it has one.
	private func progressState(of tab: AppTab) -> Any {
		guard surfaces.tabStacks[tab.id]?.find(CreationProgressPage.layerName) != nil else { return NSNull() }
		let page = CreationPageState(store: store, tab: tab.id)
		let progress = page.progress
		return [
			"title": page.title,
			"steps": progress.steps.map { ["label": $0.label, "state": "\($0.state)"] },
			"outputLines": progress.output.count,
			"failure": page.failure ?? NSNull(),
			"action": page.action,
		] as [String: Any]
	}

	private func dialogState() -> Any {
		guard let dialog = surfaces.dialog else { return NSNull() }
		let confirmation = dialog.confirmation
		return [
			"title": confirmation.title,
			"intro": confirmation.intro,
			"items": confirmation.items.map { ["name": $0.name, "detail": $0.detail] },
			"confirmLabel": confirmation.confirmLabel,
			"focused": dialog.focused == .confirm ? "confirm" : "cancel",
		] as [String: Any]
	}

	private func menuState(of hub: WorkspaceHubModel) -> Any {
		guard let menu = hub.openMenu else { return NSNull() }
		let items: [[String: Any]] = hub.menuItems(menu).filter { $0.kind != .separator }.map { item in
			[
				"kind": item.kind.rawValue, "label": item.label, "detail": item.detail ?? NSNull(),
				"checked": item.checked, "enabled": item.enabled,
			]
		}
		return ["kind": menu.rawValue, "items": items] as [String: Any]
	}

	private func hubState() -> Any {
		guard let hub = surfaces.hub else { return NSNull() }
		let focused = hub.focused
		return [
			"profileLabel": store.profiles.first { $0.path == store.selectedProfilePath }?.name ?? NSNull(),
			"menu": menuState(of: hub),
			"loading": !hub.loaded,
			"welcome": hub.welcome != nil,
			"category": hub.category.label,
			"filter": hub.filter,
			"filterFocused": surfaces.router.isEditingText,
			"items": hub.items.map { ["id": $0.id, "title": $0.title, "dot": $0.dot(status: hub.status(of: $0)).rawValue] },
			"focused": focused?.id ?? NSNull(),
			"detail": focused.map { workspace -> [String: Any] in
				let status = hub.status(of: workspace)
				return [
					"id": workspace.id,
					"error": status?.error ?? NSNull(),
					"repos": status.map { status in
						status.repos.map {
							["name": $0.name, "branch": $0.branch, "detached": $0.isDetached, "modified": $0.modified, "untracked": $0.untracked]
						}
					} ?? NSNull(),
				]
			} ?? NSNull(),
			"toast": hub.toast ?? NSNull(),
			"cheatsheet": surfaces.cheatsheet.map { scroll -> [String: Any] in
				let geometry = scroll.geometry
				return ["offset": geometry.offset, "contentHeight": geometry.contentHeight, "viewportHeight": geometry.viewportHeight]
			} ?? NSNull(),
		] as [String: Any]
	}

	private func state() -> [String: Any] {
		let window = store.app.window
		return [
			"hub": hubState(),
			"dialog": dialogState(),
			"newTask": newTaskState(),
			"profileForm": profileFormState(),
			"fontSize": TerminalRuntime.shared.fontSize,
			"windowLayers": surfaces.windowStack.layers.map(\.spec.name),
			"editingText": surfaces.router.isEditingText,
			"version": store.version,
			"profiles": store.profiles.map { ["name": $0.name, "path": $0.path] },
			"selectedProfile": store.selectedProfilePath ?? NSNull(),
			"workspaces": store.workspaces.map { ["id": $0.id, "title": $0.title, "path": $0.path, "active": $0.active] },
			"errorBanners": store.errorBanners.visible.map { ["id": NSNumber(value: $0.id), "message": $0.message, "count": $0.count] },
			"logPath": (try? store.log.path()) ?? NSNull(),
			"startupProblem": store.startupProblem.map(Self.describe) ?? NSNull(),
			"activeTab": window.activeTab.map { NSNumber(value: $0) } ?? NSNull(),
			"tabs": window.tabs.map { tab in
				let session = terminals.session(for: tab)
				return [
					"id": NSNumber(value: tab.id),
					"title": tab.title,
					"kind": Self.name(tab.session.kind),
					"terminal": Self.status(session),
					"focused": session?.hasFocus ?? false,
					"layers": surfaces.tabStacks[tab.id]?.layers.map(\.spec.name) ?? [],
					"progress": progressState(of: tab),
				] as [String: Any]
			},
		]
	}

	private static func describe(_ problem: StartupProblem) -> [String: Any] {
		switch problem {
		case .stateInUse(let path): ["kind": "stateInUse", "path": path]
		case .stateUnreadable(let path, let reason): ["kind": "stateUnreadable", "path": path, "reason": reason]
		}
	}

	private static func name(_ kind: SessionKind) -> String {
		switch kind {
		case .shell: "shell"
		case .workspace: "workspace"
		case .creatingWorkspace: "creatingWorkspace"
		case .creationFailed: "creationFailed"
		}
	}

	private static func status(_ session: TerminalSession?) -> String {
		guard let session else { return "none" }
		if let reason = session.startFailure { return "failed: \(reason)" }
		return session.isRunning ? "running" : "starting"
	}
}

/// A key the bridge can press, by the name the harness uses: a letter or a digit, `Minus`, `Equal`, `Slash`, `Space`, `Left`, `Right`, `Down`, `Up`, `Escape`, `Tab` or `Return`.
private struct TestKey {
	let keyCode: UInt16
	/// What the key types unshifted, and with Shift (on a US layout).
	let plain: String
	let shifted: String
	/// Arrows are function keys on the numeric pad, as far as AppKit's flags go.
	var isArrow = false

	init?(_ name: String) {
		let named: [String: TestKey] = [
			"Minus": TestKey(keyCode: KeyCode.minus, plain: "-", shifted: "_"),
			"Equal": TestKey(keyCode: KeyCode.equal, plain: "=", shifted: "+"),
			"Left": TestKey(keyCode: KeyCode.leftArrow, plain: "\u{F702}", shifted: "\u{F702}", isArrow: true),
			"Right": TestKey(keyCode: KeyCode.rightArrow, plain: "\u{F703}", shifted: "\u{F703}", isArrow: true),
			"Down": TestKey(keyCode: KeyCode.downArrow, plain: "\u{F701}", shifted: "\u{F701}", isArrow: true),
			"Up": TestKey(keyCode: KeyCode.upArrow, plain: "\u{F700}", shifted: "\u{F700}", isArrow: true),
			"Escape": TestKey(keyCode: KeyCode.escape, plain: "\u{1B}", shifted: "\u{1B}"),
			"Tab": TestKey(keyCode: KeyCode.tab, plain: "\t", shifted: "\t"),
			"Return": TestKey(keyCode: KeyCode.returnKey, plain: "\r", shifted: "\r"),
			"Space": TestKey(keyCode: 0x31, plain: " ", shifted: " "),
			"Slash": TestKey(keyCode: 0x2C, plain: "/", shifted: "?"),
		]
		// kVK_ANSI_* codes follow the keyboard's physical layout, not the alphabet.
		let letters: [Character: UInt16] = [
			"a": 0x00, "s": 0x01, "d": 0x02, "f": 0x03, "h": 0x04, "g": 0x05, "z": 0x06, "x": 0x07, "c": 0x08, "v": 0x09,
			"b": 0x0B, "q": 0x0C, "w": 0x0D, "e": 0x0E, "r": 0x0F, "y": 0x10, "t": 0x11, "o": 0x1F, "u": 0x20, "i": 0x22,
			"p": 0x23, "l": 0x25, "j": 0x26, "k": 0x28, "n": 0x2D, "m": 0x2E,
		]
		let digits: [Character: (UInt16, String)] = [
			"1": (0x12, "!"), "2": (0x13, "@"), "3": (0x14, "#"), "4": (0x15, "$"), "5": (0x17, "%"),
			"6": (0x16, "^"), "7": (0x1A, "&"), "8": (0x1C, "*"), "9": (0x19, "("), "0": (0x1D, ")"),
		]
		if let key = named[name] {
			self = key
		} else if name.count == 1, let c = name.first, let code = letters[c] {
			self.init(keyCode: code, plain: String(c), shifted: c.uppercased())
		} else if name.count == 1, let c = name.first, let (code, shifted) = digits[c] {
			self.init(keyCode: code, plain: String(c), shifted: shifted)
		} else {
			return nil
		}
	}

	private init(keyCode: UInt16, plain: String, shifted: String, isArrow: Bool = false) {
		self.keyCode = keyCode
		self.plain = plain
		self.shifted = shifted
		self.isArrow = isArrow
	}

	/// The key-down AppKit would make: `charactersIgnoringModifiers` keeps Shift, and Control turns a letter into its control character.
	@MainActor
	func event(modifiers: KeyModifiers, window: NSWindow) throws -> NSEvent {
		var flags: NSEvent.ModifierFlags = []
		if modifiers.contains(.shift) { flags.insert(.shift) }
		if modifiers.contains(.control) { flags.insert(.control) }
		if modifiers.contains(.option) { flags.insert(.option) }
		if modifiers.contains(.command) { flags.insert(.command) }
		if isArrow { flags.formUnion([.function, .numericPad]) }
		let ignoringModifiers = modifiers.contains(.shift) ? shifted : plain
		var characters = ignoringModifiers
		if modifiers.contains(.control), let scalar = ignoringModifiers.unicodeScalars.first, scalar.properties.isAlphabetic, scalar.isASCII {
			characters = String(UnicodeScalar(UInt8(scalar.value) & 0x1F))
		}
		guard let event = NSEvent.keyEvent(
			with: .keyDown, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
			windowNumber: window.windowNumber, context: nil, characters: characters,
			charactersIgnoringModifiers: ignoringModifiers, isARepeat: false, keyCode: keyCode)
		else { throw BridgeError(status: 500, "couldn't make a key event") }
		return event
	}
}

private struct BridgeError: Error {
	let status: Int
	let message: String

	init(status: Int, _ message: String) {
		self.status = status
		self.message = message
	}
}

private struct HTTPRequest: Sendable {
	let method: String
	let path: String
	let query: [String: String]
	let body: Data

	func json() throws -> [String: Any] {
		guard !body.isEmpty else { return [:] }
		guard let object = try JSONSerialization.jsonObject(with: body) as? [String: Any] else {
			throw BridgeError(status: 400, "the body isn't a JSON object")
		}
		return object
	}
}

private struct HTTPResponse: @unchecked Sendable {
	let status: Int
	let body: [String: Any]
}

/// One request and its response on a connection: the harness's `fetch` sends a single request per connection.
private final class HTTPExchange: @unchecked Sendable {
	private let connection: NWConnection
	private var buffer = Data()

	init(connection: NWConnection) {
		self.connection = connection
	}

	func receive(_ handle: @escaping @Sendable (HTTPRequest) async -> HTTPResponse) {
		connection.receive(minimumIncompleteLength: 1, maximumLength: 1 << 16) { data, _, isComplete, error in
			if let data { self.buffer.append(data) }
			if let request = self.parse() {
				Task {
					let response = await handle(request)
					self.send(response)
				}
			} else if isComplete || error != nil {
				self.connection.cancel()
			} else {
				self.receive(handle)
			}
		}
	}

	/// The request, once its headers and its `Content-Length` of body have arrived.
	private func parse() -> HTTPRequest? {
		guard let headerEnd = buffer.firstRange(of: Data("\r\n\r\n".utf8)) else { return nil }
		let lines = String(decoding: buffer[..<headerEnd.lowerBound], as: UTF8.self).components(separatedBy: "\r\n")
		let requestLine = lines[0].split(separator: " ")
		guard requestLine.count >= 2 else { return nil }
		var length = 0
		for line in lines.dropFirst() {
			let parts = line.split(separator: ":", maxSplits: 1)
			if parts.count == 2, parts[0].lowercased() == "content-length" {
				length = Int(parts[1].trimmingCharacters(in: .whitespaces)) ?? 0
			}
		}
		let body = buffer[headerEnd.upperBound...]
		guard body.count >= length else { return nil }
		let components = URLComponents(string: String(requestLine[1]))
		var query: [String: String] = [:]
		for item in components?.queryItems ?? [] { query[item.name] = item.value }
		return HTTPRequest(
			method: String(requestLine[0]), path: components?.path ?? "", query: query, body: Data(body.prefix(length)))
	}

	private func send(_ response: HTTPResponse) {
		let body = (try? JSONSerialization.data(withJSONObject: response.body)) ?? Data("{}".utf8)
		let head = "HTTP/1.1 \(response.status) \(response.status == 200 ? "OK" : "Error")\r\nContent-Type: application/json\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n"
		connection.send(content: Data(head.utf8) + body, completion: .contentProcessed { _ in self.connection.cancel() })
	}
}
