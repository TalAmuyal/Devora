import AppKit
import CoreBindings
import Foundation
import Network
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
	private let listener: NWListener

	/// Starts the bridge, unless it is running, and prints its port to stderr, where the harness reads it, as Ember does.
	static func start(store: Store, terminals: Terminals) {
		guard running == nil else { return }
		do {
			let bridge = try TestBridge(store: store, terminals: terminals)
			running = bridge
			bridge.listen()
		} catch {
			NSLog("test bridge: failed to start: %@", String(describing: error))
		}
	}

	private init(store: Store, terminals: Terminals) throws {
		self.store = store
		self.terminals = terminals
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
		case "activateTab": command = .activateTab(id: try tabID(body))
		case "closeTab": command = .closeTab(id: try tabID(body))
		case "startFresh": command = .startFresh
		default:
			throw BridgeError(status: 400, "unknown command \(String(describing: body["command"]))")
		}
		await store.dispatch(command).value
	}

	private func tabID(_ body: [String: Any]) throws -> UInt64 {
		guard let id = (body["id"] as? NSNumber)?.uint64Value else { throw BridgeError(status: 400, "missing id") }
		return id
	}

	/// The terminal of the tab with this id (the active tab by default); a 409 until it is running, so that callers poll.
	private func terminal(_ id: UInt64?) throws -> TerminalSession {
		let window = store.app.window
		guard let id = id ?? window.activeTab, let tab = window.tabs.first(where: { $0.id == id }) else {
			throw BridgeError(status: 404, "no tab \(id.map(String.init) ?? "is active")")
		}
		guard let session = terminals.session(for: tab) else { throw BridgeError(status: 409, "the terminal of tab \(id) isn't running") }
		return session
	}

	private func state() -> [String: Any] {
		let window = store.app.window
		return [
			"version": store.version,
			"profiles": store.profiles.map { ["name": $0.name, "path": $0.path] },
			"selectedProfile": store.selectedProfilePath ?? NSNull(),
			"workspaces": store.workspaces.map { ["id": $0.id, "title": $0.title, "path": $0.path, "active": $0.active] },
			"error": store.error ?? NSNull(),
			"startupProblem": store.startupProblem.map(Self.describe) ?? NSNull(),
			"warnings": store.warnings,
			"activeTab": window.activeTab.map { NSNumber(value: $0) } ?? NSNull(),
			"tabs": window.tabs.map { tab in
				let session = terminals.session(for: tab)
				return [
					"id": NSNumber(value: tab.id),
					"title": tab.title,
					"kind": Self.name(tab.session.kind),
					"terminal": Self.name(terminals.slots[tab.terminalID]),
					"focused": session?.hasFocus ?? false,
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

	private static func name(_ slot: Terminals.Slot?) -> String {
		switch slot {
		case nil: "none"
		case .starting: "starting"
		case .failed(let reason): "failed: \(reason)"
		case .running: "running"
		}
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
