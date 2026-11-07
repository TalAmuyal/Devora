import AppKit
import CoreBindings
import Foundation
import Layers
import Network
import WebKit
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
			case ("GET", "/test/guide"):
				guard let model = surfaces.userGuide, let view = model.webView else { throw BridgeError(status: 409, "guide web view is not ready") }
				let text = try await view.evaluateJavaScript("document.body?.textContent ?? ''")
				return .init(status: 200, body: ["text": text as? String ?? "", "error": model.error ?? NSNull()])
			case ("GET", "/test/preview"):
				guard let id = request.query["pane"].flatMap(UInt64.init), let view = surfaces.previewModels[id]?.webView else { throw BridgeError(status: 409, "preview web view is not ready") }
				let rendered = try await view.evaluateJavaScript("""
					(() => { const svg = document.querySelector('.mermaid-diagram svg'); const shape = svg?.querySelector('.node rect, rect.basic, .node polygon, .node path'); return { heading: document.querySelector('h1')?.textContent ?? '', mermaid: svg?.textContent ?? '', fill: shape ? getComputedStyle(shape).fill : '', scriptRan: document.body?.dataset.scriptRan ?? '', fileStyle: getComputedStyle(document.body).getPropertyValue('--file-loaded').trim(), images: Array.from(document.images).map(i => ({ complete: i.complete, width: i.naturalWidth })) }; })()
					""")
				var observation = rendered as? [String: Any] ?? [:]
				observation["htmlSandbox"] = !view.configuration.websiteDataStore.isPersistent
					&& !view.configuration.defaultWebpagePreferences.allowsContentJavaScript
					&& view.configuration.urlSchemeHandler(forURLScheme: "devora-renderer") == nil
				observation["url"] = view.url?.absoluteString ?? ""
				return .init(status: 200, body: observation)
			case ("POST", "/test/preview/action"):
				let body = try request.json()
				let pane = try uint64(body, "pane")
				guard let view = surfaces.previewModels[pane]?.webView else { throw BridgeError(status: 409, "preview web view is not ready") }
				let actions = ["link": "document.getElementById('remote-link').click()", "popup": "document.getElementById('popup-link').click()", "file": "document.getElementById('file-link').click()", "renderer": "document.getElementById('renderer-link').click()", "form": "document.getElementById('remote-form').requestSubmit()"]
				guard let action = body["action"] as? String, let script = actions[action] else { throw BridgeError(status: 400, "unknown preview action") }
				_ = try await view.evaluateJavaScript(script)
				return .init(status: 200, body: [:])
			case ("POST", "/test/preview/close"):
				let body = try request.json()
				let pane = try uint64(body, "pane")
				guard let tab = store.app.window.tabs.first(where: { $0.previews.contains(where: { $0.id == pane }) }) else { throw BridgeError(status: 404, "preview not found") }
				await store.dispatch(.closePreview(tab: tab.id, pane: pane)).value
				surfaces.router.focusActiveTab()
				return .init(status: 200, body: [:])
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
			case ("POST", "/test/palette"):
				try paletteAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/repurpose"):
				try repurposeAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/hub"):
				try hubAction(request.json())
				return .init(status: 200, body: [:])
			case ("GET", "/test/agent-chat"):
				guard let tab = store.app.window.activeTab, let panel = surfaces.agentPanels[tab], let view = panel.webView else { throw BridgeError(status: 409, "agent chat is not ready") }
				let dom = try await view.evaluateJavaScript("""
					(() => {
					  const content = document.getElementById('content');
					  const rows = [...document.querySelectorAll('article[data-sequence]')];
					  const visible = rows.find(row => row.offsetTop + row.offsetHeight > content.scrollTop);
					  return {
					    text: content.textContent,
					    rows: rows.map(row => ({
					      sequence: row.dataset.sequence, kind: row.className,
					      text: row.textContent, offset: row.offsetTop - content.scrollTop
					    })),
					    cards: document.querySelectorAll('.chat-permission').length,
					    thinkingOpen: document.querySelector('.chat-thinking details')?.open ?? false,
					    toolOpen: document.querySelector('.chat-tool details')?.open ?? false,
					    verdict: document.querySelector('.chat-verdict')?.textContent ?? '',
					    scripts: document.querySelectorAll('article script').length,
					    unsafe: document.querySelectorAll('article [onerror], article [onclick]').length,
					    top: content.scrollTop, topSequence: visible?.dataset.sequence ?? null
					  };
					})()
					""")
				var observation: [String: Any] = ["dom": dom ?? [:], "draft": panel.chat.draft, "canSend": panel.chat.canSend]
				observation["phase"] = panel.chat.chat.map { String(describing: $0.phase) } ?? "opening"
				observation["context"] = panel.chat.chat?.context.map { ["used": String($0.usedTokens), "window": String($0.windowTokens), "threshold": $0.thresholdPercent] as [String: Any] } ?? NSNull()
				observation["enterSends"] = panel.chat.enterSends
				observation["running"] = panel.chat.chat?.turnRunning ?? false
				observation["anchor"] = panel.chat.anchor.map { String($0.sequence) as Any } ?? NSNull()
				observation["questions"] = panel.chat.chat?.questions.count ?? 0
				observation["error"] = (panel.chat.error ?? panel.rendererError).map { $0 as Any } ?? NSNull()
				return .init(status: 200, body: observation)
			case ("POST", "/test/agent-chat"):
				guard let tab = store.app.window.activeTab, let panel = surfaces.agentPanels[tab] else { throw BridgeError(status: 409, "agent chat is not open") }
				let body = try request.json()
				switch body["action"] as? String {
				case "draft": panel.chat.draft = body["text"] as? String ?? ""; panel.chat.flushDraft()
				case "send": await panel.chat.send()
				case "stop": await panel.chat.stop()
				case "pause": panel.chat.pause()
				case "retry": await panel.chat.retry()
				case "older": await panel.chat.older()
				case "toggleThinking": _ = try await panel.webView?.evaluateJavaScript("document.querySelector('.chat-thinking summary')?.click()")
				case "toggleTool": _ = try await panel.webView?.evaluateJavaScript("document.querySelector('.chat-tool summary')?.click()")
				case "scrollTop": _ = try await panel.webView?.evaluateJavaScript("document.getElementById('content').scrollTo({top:0})")
				default: throw BridgeError(status: 400, "unknown chat action")
				}
				return .init(status: 200, body: [:])
			case ("POST", "/test/agent-hub"):
				try agentHubAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/new-agent"):
				try newAgentAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/dialog"):
				try dialogButton(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/new-task"):
				try newTaskAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/add-repo"):
				try addRepoAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/clone-repo"):
				try cloneRepoAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/profile-form"):
				try profileFormAction(request.json())
				return .init(status: 200, body: [:])
			case ("POST", "/test/settings"):
				try settingsAction(request.json())
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

	private func paletteAction(_ body: [String: Any]) throws {
		switch body["action"] as? String {
		case "shiftShift":
			let now = ProcessInfo.processInfo.systemUptime
			for (offset, flags): (Double, KeyModifiers) in [(0, .shift), (0.05, []), (0.1, .shift), (0.15, [])] {
				surfaces.shortcuts.handleFlags(keyCode: KeyCode.leftShift, modifiers: flags, at: now + offset)
			}
		case "filter":
			guard let palette = surfaces.palette, let value = body["value"] as? String else { throw BridgeError(status: 409, "palette or value missing") }
			palette.filter = value
		case "blur":
			guard let palette = surfaces.palette else { throw BridgeError(status: 409, "palette is not open") }
			palette.fieldFocused = false
			surfaces.router.clearFocus()
		default: throw BridgeError(status: 400, "unknown palette action")
		}
	}

	private func repurposeAction(_ body: [String: Any]) throws {
		guard let dialog = surfaces.repurposeDialog else { throw BridgeError(status: 409, "repurpose dialog is not open") }
		switch body["action"] as? String {
		case "title":
			guard let value = body["value"] as? String else { throw BridgeError(status: 400, "missing title") }
			dialog.title = value
		case "confirm": dialog.confirm()
		case "cancel": dialog.close()
		default: throw BridgeError(status: 400, "unknown repurpose action")
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
		case "resizePreview": command = .resizePreview(tab: try uint64(body, "tab"), pane: try uint64(body, "pane"), width: UInt32(clamping: try uint64(body, "width")))
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
		case "duplicate", "addRepo":
			guard let workspace = hub.focused, workspace.active else { throw BridgeError(status: 409, "no active workspace is focused") }
			if body["action"] as? String == "duplicate" { hub.newTask(workspace) } else { hub.addRepo(workspace) }
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
		case "cloneRepo": dialog.cloneRepo()
		default: throw BridgeError(status: 400, "unknown New Task action \(String(describing: body["action"]))")
		}
	}

	/// Clicks what the open Add Repo dialog draws: a repo's row (by its name), or one of its buttons, the progress's included (`progressAction`: Cancel, or Close once the add has failed).
	private func addRepoAction(_ body: [String: Any]) throws {
		guard let dialog = surfaces.addRepo else { throw BridgeError(status: 409, "the Add Repo dialog isn't open") }
		switch body["action"] as? String {
		case "selectRepo":
			let name = body["name"] as? String
			guard let repo = dialog.visibleRepos.first(where: { $0.name == name }) else {
				throw BridgeError(status: 404, "no repo \(name ?? "nil") in the Add Repo dialog's list")
			}
			dialog.click(repo)
		case "submit": dialog.submit()
		case "cancel": dialog.cancel()
		case "progressAction":
			guard let operation = dialog.operation else { throw BridgeError(status: 409, "the add hasn't started") }
			operation.closeOrCancel()
		default: throw BridgeError(status: 400, "unknown Add Repo action \(String(describing: body["action"]))")
		}
	}

	/// Fills in the open Clone Repo dialog's URL, as typing it does (`setUrl` with a `value`), or clicks its buttons, the progress's included (`progressAction`).
	private func cloneRepoAction(_ body: [String: Any]) throws {
		guard let dialog = surfaces.cloneRepo else { throw BridgeError(status: 409, "the Clone Repo dialog isn't open") }
		switch body["action"] as? String {
		case "setUrl":
			guard let value = body["value"] as? String else { throw BridgeError(status: 400, "missing value") }
			dialog.url = value
		case "submit": dialog.submit()
		case "cancel": dialog.cancel()
		case "progressAction":
			guard let operation = dialog.operation else { throw BridgeError(status: 409, "the clone hasn't started") }
			operation.closeOrCancel()
		default: throw BridgeError(status: 400, "unknown Clone Repo action \(String(describing: body["action"]))")
		}
	}

	private static func operationState(_ operation: RepoOperationPhase?) -> Any {
		guard let operation else { return NSNull() }
		return [
			"title": operation.title,
			"steps": operation.run.progress.steps.map { ["label": $0.label, "state": "\($0.state)"] },
			"failure": operation.failure ?? NSNull(),
			"action": operation.action,
		] as [String: Any]
	}

	private func addRepoState() -> Any {
		guard let dialog = surfaces.addRepo else { return NSNull() }
		let selected = dialog.selectedRepo
		return [
			"workspacePath": dialog.target.workspacePath,
			"filter": dialog.filter,
			"postfix": dialog.postfix,
			"focus": "\(dialog.focus)",
			"repos": dialog.visibleRepos.map { ["name": $0.name, "selected": $0 == selected] },
			"progress": Self.operationState(dialog.operation),
		] as [String: Any]
	}

	private func cloneRepoState() -> Any {
		guard let dialog = surfaces.cloneRepo else { return NSNull() }
		return [
			"url": dialog.url,
			"hint": dialog.hint ?? NSNull(),
			"focus": "\(dialog.focus)",
			"progress": Self.operationState(dialog.operation),
		] as [String: Any]
	}

	/// The open profile form: the Settings Hub's New Profile row's, or else the first-run welcome's.
	private var profileForm: ProfileFormModel? {
		surfaces.settings?.form ?? surfaces.hub?.welcome
	}

	/// Clicks what the open Settings Hub draws: a master row (`selectRow` by its label), a profile detail's button (`setActive`, `cloneRepo`, `delete`), or a settings card's control on the focused row's scope:
	/// a segment (`selectSegment` with a `key` and the segment's `label`), typing into a revealed input (`type` with a `key` and a `value`), Enter in it (`commit`), or its Save button (`save`, a 409 while it is hidden).
	private func settingsAction(_ body: [String: Any]) throws {
		guard let settings = surfaces.settings else { throw BridgeError(status: 409, "the Settings Hub isn't open") }
		let action = body["action"] as? String
		switch action {
		case "selectRow":
			let label = body["label"] as? String
			guard let row = settings.rows.first(where: { Self.label(of: $0) == label }) else {
				throw BridgeError(status: 404, "no row \(label ?? "nil") in the Settings Hub")
			}
			settings.select(row)
			return
		case "setActive", "cloneRepo", "delete":
			guard case .profile(let profile) = settings.focusedRow else { throw BridgeError(status: 409, "no profile is focused") }
			switch action {
			case "setActive": settings.setActive(profile)
			case "cloneRepo": settings.cloneRepo(into: profile)
			default: settings.delete(profile)
			}
			return
		default:
			break
		}
		guard let scope = settings.scope, scope.settings != nil else {
			throw BridgeError(status: 409, "the focused row's settings aren't loaded")
		}
		let key = body["key"] as? String
		if key == "asana-token" {
			guard scope.usesAsana, let token = scope.asana else { throw BridgeError(status: 404, "the Asana token row isn't shown") }
			switch action {
			case "type":
				guard let value = body["value"] as? String else { throw BridgeError(status: 400, "missing value") }
				token.draft = value
			case "save", "commit":
				guard token.canSave else { throw BridgeError(status: 409, "the token has no value to save") }
				token.saveToken()
			case "clear":
				guard token.isSet == true else { throw BridgeError(status: 409, "the token isn't set") }
				token.clearToken()
			default: throw BridgeError(status: 400, "unknown token action")
			}
			return
		}
		if let key, ["opus-model", "sonnet-model", "haiku-model", "effort", "default-model", "permission-mode"].contains(key) {
			guard let claude = scope.claude, claude.settings != nil, let choices = claude.choices else { throw BridgeError(status: 409, "Claude settings aren't loaded") }
			let label = body["label"] as? String
			switch action {
			case "selectSegment":
				guard let label else { throw BridgeError(status: 400, "missing segment label") }
				if key == "effort" {
					guard (["Default", "None"] + choices.effortLevels).contains(label) else { throw BridgeError(status: 404, "no effort segment \(label)") }
					claude.selectEffort(label)
				} else if key == "default-model" {
					guard ["On", "Off"].contains(label) else { throw BridgeError(status: 404, "no default-model segment \(label)") }
					claude.setDefaultModel(label == "On")
				} else {
					guard let mode = ClaudeMode.allCases.first(where: { $0.label == label }) else { throw BridgeError(status: 404, "no Claude segment \(label)") }
					claude.select(mode, key: key)
				}
			case "type", "commit", "save", "suggest":
				guard key != "effort", key != "default-model", claude.mode(key) == .custom else { throw BridgeError(status: 409, "the Claude input isn't shown") }
				if action == "type" {
					guard let value = body["value"] as? String else { throw BridgeError(status: 400, "missing value") }
					claude.edit(key, value)
				} else {
					if action == "save", !claude.showsSave(key) { throw BridgeError(status: 409, "the Claude Save button isn't shown") }
					if action == "suggest" {
						let suggestions = key == "permission-mode" ? choices.permissionModeSuggestions : choices.modelSuggestions
						guard let label, suggestions.contains(label) else { throw BridgeError(status: 404, "no matching suggestion") }
						claude.edit(key, label)
					}
					claude.commit(key)
				}
			default: throw BridgeError(status: 400, "unknown Claude setting action")
			}
			return
		}
		if ["selectSegment", "type", "save", "commit"].contains(action ?? ""),
			let key, key.hasPrefix("agent."),
			let field = AgentSettingsField.all.first(where: { "agent.\($0.key)" == key })
		{
			guard let agent = scope.agent, agent.settings != nil else { throw BridgeError(status: 409, "Agent settings aren't loaded") }
			switch action {
			case "selectSegment":
				guard let mode = TextMode.allCases.first(where: { $0.label == body["label"] as? String }) else {
					throw BridgeError(status: 400, "unknown Agent setting segment")
				}
				if agent.mode(field.key) != mode { agent.select(mode, key: field.key) }
			case "type":
				guard agent.mode(field.key) == .set, let value = body["value"] as? String else { throw BridgeError(status: 409, "Agent input isn't shown") }
				agent.edit(field.key, value)
			case "save", "commit":
				guard agent.mode(field.key) == .set, agent.showsSave(field) else { throw BridgeError(status: 409, "Agent setting has no Save button") }
				agent.commit(field.key)
			default: throw BridgeError(status: 400, "unknown Agent field action")
			}
			return
		}
		if ["addAgentInput", "saveAgentInput", "defaultAgentInput"].contains(action ?? "") {
			guard let agent = scope.agent, agent.settings != nil, let key else { throw BridgeError(status: 409, "Agent inputs aren't loaded") }
			if action == "addAgentInput" {
				guard agent.settings?.inputsCleared == false, !agent.savingInputs.contains("$new") else { throw BridgeError(status: 409, "Agent input addition isn't available") }
				agent.newInput = .init(key: key, value: body["value"] as? String ?? "")
				agent.addInput()
			} else {
				guard let entry = agent.settings?.inputs.first(where: { $0.key == key }), !agent.savingInputs.contains(key) else { throw BridgeError(status: 404, "Agent input isn't shown") }
				if action == "defaultAgentInput" {
					guard entry.stored != nil else { throw BridgeError(status: 409, "Agent input already inherits") }
					agent.defaultInput(entry)
				} else {
					if let newKey = body["newKey"] as? String, newKey != key, entry.stored == nil { throw BridgeError(status: 409, "An inherited key cannot be renamed") }
					agent.editInput(entry, key: body["newKey"] as? String, value: body["value"] as? String)
					agent.saveInput(entry)
				}
			}
			return
		}
		guard let field = ConfigCard.all.flatMap(\.fields).first(where: { $0.key == key }), scope.isVisible(field) else {
			throw BridgeError(status: 404, "no setting \(key ?? "nil") is shown")
		}
		let label = body["label"] as? String
		let noSegment = BridgeError(status: 404, "\(field.key) has no segment \(label ?? "nil")")
		switch (action, field.kind) {
		case ("selectSegment", .text):
			guard let mode = TextMode.allCases.first(where: { $0.label == label }) else { throw noSegment }
			if mode != scope.textMode(field.key) { scope.selectTextMode(mode, for: field.key) }
		case ("selectSegment", .bool):
			guard let choice = BoolChoice.allCases.first(where: { $0.label == label }) else { throw noSegment }
			if choice != scope.boolChoice(field.key) { scope.selectBool(choice, for: field.key) }
		case ("selectSegment", .choice(let options)):
			guard let option = options.first(where: { $0.label == label }) else { throw noSegment }
			if option != scope.chosenOption(options, for: field.key) { scope.selectOption(option, for: field.key) }
		case ("type", .text):
			guard scope.textMode(field.key) == .set else { throw BridgeError(status: 409, "\(field.key) has no input") }
			guard let value = body["value"] as? String else { throw BridgeError(status: 400, "missing value") }
			scope.edit(field.key, to: value)
		case ("commit", .text(_, let whitespace)), ("save", .text(_, let whitespace)):
			guard scope.textMode(field.key) == .set else { throw BridgeError(status: 409, "\(field.key) has no input") }
			if action == "save", !scope.showsSave(field.key, whitespace: whitespace) {
				throw BridgeError(status: 409, "\(field.key) has no Save button")
			}
			scope.commit(field.key, whitespace: whitespace)
		default:
			throw BridgeError(status: 400, "unknown settings action \(action ?? "nil") for \(field.key)")
		}
	}

	private static func label(of row: SettingsRow) -> String {
		switch row {
		case .userDefaults: "User Defaults"
		case .profile(let profile): profile.name
		case .newProfile: "New Profile…"
		}
	}

	/// The open Settings Hub: its rows, the focused one's detail, and the settings of its scope as the cards show them.
	private func settingsState() -> Any {
		guard let settings = surfaces.settings else { return NSNull() }
		let focused = settings.focusedRow
		var detail: Any = NSNull()
		if case .profile(let profile) = focused {
			let loaded = settings.details[profile.path]
			detail = [
				"name": profile.name,
				"path": profile.path,
				"active": settings.isActive(profile),
				"workspaceCount": loaded?.workspaceCount ?? NSNull(),
				"activeTaskCount": loaded?.activeTaskCount ?? NSNull(),
				"repos": loaded.map { $0.repos.map { ["name": $0.name, "source": $0.source, "path": $0.path] } } ?? NSNull(),
			] as [String: Any]
		}
		return [
			"loaded": settings.loaded,
			"rows": settings.rows.map { row -> [String: Any] in
				var state: [String: Any] = ["label": Self.label(of: row), "focused": row.id == focused.id]
				switch row {
				case .userDefaults: state["kind"] = "userDefaults"
				case .newProfile: state["kind"] = "newProfile"
				case .profile(let profile):
					state["kind"] = "profile"
					state["active"] = settings.isActive(profile)
					state["repoCount"] = profile.repoCount
				}
				return state
			},
			"focused": Self.label(of: focused),
			"detail": detail,
			"scope": settings.scope.map(scopeState) ?? NSNull(),
		] as [String: Any]
	}

	/// The settings of a scope as the cards show them: each shown field's segment, its input while revealed, its Save button and its inherited hint.
	private func scopeState(_ scope: ConfigSettingsModel) -> Any {
		var state: [String: Any] = ["profilePath": scope.profilePath ?? NSNull(), "loaded": scope.settings != nil]
		guard scope.settings != nil else { return state }
		state["fields"] = ConfigCard.all.flatMap(\.fields).filter(scope.isVisible).map { field -> [String: Any] in
			var shown: [String: Any] = ["key": field.key, "hint": scope.inheritedHint(field) ?? NSNull()]
			switch field.kind {
			case .text(_, let whitespace):
				let mode = scope.textMode(field.key)
				shown["segment"] = mode.label
				shown["input"] = mode == .set ? scope.inputText(field.key) : NSNull()
				shown["showsSave"] = mode == .set && scope.showsSave(field.key, whitespace: whitespace)
			case .bool:
				shown["segment"] = scope.boolChoice(field.key).label
			case .choice(let options):
				shown["segment"] = scope.chosenOption(options, for: field.key)?.label ?? NSNull()
			}
			return shown
		}
		if let agent = scope.agent, let settings = agent.settings {
			let fields = AgentSettingsField.all.map { field -> [String: Any] in
				let mode = agent.mode(field.key)
				return [
					"key": "agent.\(field.key)", "segment": mode.label,
					"input": mode == .set ? agent.input(field.key) : NSNull(),
					"showsSave": mode == .set && agent.showsSave(field),
					"hint": agent.entry(field.key).map { agent.hint($0) } ?? NSNull(),
				]
			}
			state["fields"] = (state["fields"] as? [[String: Any]] ?? []) + fields
			state["agent"] = [
				"loaded": true, "inputsCleared": settings.inputsCleared,
				"inputs": settings.inputs.map { entry in
					["key": entry.key, "inputKey": agent.inputDraft(entry).key,
					 "value": agent.inputDraft(entry).value, "stored": entry.stored != nil,
					 "hint": agent.hint(entry)] as [String: Any]
				},
			]
		}
		if let claude = scope.claude {
			state["claude"] = [
				"loaded": claude.settings != nil && claude.choices != nil,
				"effort": claude.effort,
				"effortLevels": Array((claude.choices?.effortLevels ?? []).reversed()),
				"defaultModelOn": claude.defaultModelOn,
				"fields": ["opus-model", "sonnet-model", "haiku-model", "permission-mode"].map { key -> [String: Any] in
					let mode = claude.mode(key)
					return [
						"key": key, "segment": mode.label, "hint": claude.inheritedHint(key),
						"input": mode == .custom ? claude.input(key) : NSNull(),
						"showsSave": mode == .custom && claude.showsSave(key),
						"suggestions": (key == "permission-mode" ? claude.choices?.permissionModeSuggestions : claude.choices?.modelSuggestions) ?? [],
					]
				},
			] as [String: Any]
		}
		if scope.usesAsana, let token = scope.asana {
			state["asana"] = ["isSet": token.isSet ?? NSNull(), "canSave": token.canSave, "hasDraft": !token.draft.isEmpty] as [String: Any]
		}
		return state
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

	/// What the Agent Hub draws: its agents, the focused one, and whether the first listing is done.
	private func agentHubState() -> Any {
		guard let hub = surfaces.agentHub else { return NSNull() }
		return [
			"loaded": hub.loaded,
			"focused": hub.focused?.name ?? NSNull(),
			"items": hub.items.map { ["name": $0.name, "role": $0.role ?? NSNull(), "error": $0.error ?? NSNull()] as [String: Any] },
		] as [String: Any]
	}

	/// Clicks what the Agent Hub draws: `newAgent`, or `open` and `delete` on the agent named `name` (which also focuses it).
	private func agentHubAction(_ body: [String: Any]) throws {
		guard let hub = surfaces.agentHub else { throw BridgeError(status: 409, "the Agent Hub isn't open") }
		let action = body["action"] as? String
		if action == "newAgent" {
			hub.newAgent()
			return
		}
		guard let space = hub.items.first(where: { $0.name == body["name"] as? String }) else { throw BridgeError(status: 404, "no such agent") }
		hub.select(space)
		switch action {
		case "open": hub.openFocused()
		case "delete": hub.delete(space)
		default: throw BridgeError(status: 400, "unknown agent hub action \(String(describing: action))")
		}
	}

	/// The New Agent dialog's form and its creation's progress.
	private func newAgentState() -> Any {
		guard let dialog = surfaces.newAgent else { return NSNull() }
		return [
			"name": dialog.name,
			"focus": "\(dialog.focus)",
			"keepCodingInstructions": dialog.keepCodingInstructions,
			"preview": dialog.preview.map { preview -> Any in
				switch preview {
				case .text(let text): ["text": text]
				case .failure(let error): ["error": error]
				}
			} ?? NSNull(),
			"canCreate": dialog.canCreate,
			"fields": dialog.fields.map { field in
				let value: Any = field.source == .derived ? field.value.map { $0 as Any } ?? NSNull() : dialog.value(of: field)
				return [
					"key": field.key, "value": value,
					"source": field.source.map { "\($0)" } ?? NSNull(), "saved": dialog.saved.contains(field.key),
				] as [String: Any]
			},
			"repos": dialog.visibleRepos.map { ["name": $0.name, "selected": dialog.selected.contains($0.path)] },
			"progress": dialog.run.map { run in
				["steps": run.steps, "outcome": run.outcome.map { "\($0)" } ?? NSNull()] as [String: Any]
			} ?? NSNull(),
		] as [String: Any]
	}

	/// Fills in and clicks what the New Agent dialog draws: `setName` and `setField` (with `key`) type a `value`, `toggleRepo` (with `name`), `save` (with `key`), `create` and `cancel`.
	private func newAgentAction(_ body: [String: Any]) throws {
		guard let dialog = surfaces.newAgent else { throw BridgeError(status: 409, "the New Agent dialog isn't open") }
		switch body["action"] as? String {
		case "setName": dialog.name = body["value"] as? String ?? ""
		case "setKeepCodingInstructions": dialog.keepCodingInstructions = body["value"] as? Bool ?? true
		case "setField":
			guard let key = body["key"] as? String else { throw BridgeError(status: 400, "missing key") }
			dialog.edits[key] = body["value"] as? String ?? ""
		case "toggleRepo":
			guard let repo = dialog.repos.first(where: { $0.name == body["name"] as? String }) else { throw BridgeError(status: 404, "no such repo") }
			dialog.toggle(repo)
		case "save":
			guard let field = dialog.editableFields.first(where: { $0.key == body["key"] as? String }) else { throw BridgeError(status: 404, "no such field") }
			dialog.save(field)
		case "create":
			guard dialog.canCreate else { throw BridgeError(status: 409, "Create is disabled") }
			dialog.create()
		case "cancel": dialog.cancel()
		default: throw BridgeError(status: 400, "unknown new agent action \(String(describing: body["action"]))")
		}
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
			"cancelLabel": confirmation.cancelLabel ?? NSNull(),
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

	private func healthState() -> Any {
		guard let health = surfaces.health else { return NSNull() }
		let report = health.report
		return [
			"loading": health.loading,
			"error": health.error ?? NSNull(),
			"generation": health.generation,
			"version": report?.version ?? NSNull(),
			"sections": report?.sections.map { section in
				["title": section.title, "items": section.items.map { ["name": $0.name, "message": $0.message] }]
			} ?? [],
		] as [String: Any]
	}

	private func state() -> [String: Any] {
		let window = store.app.window
		return [
			"palette": surfaces.palette.map { palette in
				["items": palette.items.map { ["id": $0.id, "title": $0.title] },
				 "selected": palette.selected?.title ?? NSNull(), "filter": palette.filter,
				 "focused": surfaces.router.isEditingText] as [String: Any]
			} ?? NSNull(),
			"repurpose": surfaces.repurposeDialog.map { ["title": $0.title] } ?? NSNull(),
			"hub": hubState(),
			"agentHub": agentHubState(),
			"newAgent": newAgentState(),
			"health": healthState(),
			"dialog": dialogState(),
			"newTask": newTaskState(),
			"addRepo": addRepoState(),
			"cloneRepo": cloneRepoState(),
			"profileForm": profileFormState(),
			"settings": settingsState(),
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
					"launchPath": session?.launch?.env["PATH"] ?? NSNull(),
					"focused": session?.hasFocus ?? false,
					"layers": surfaces.tabStacks[tab.id]?.layers.map(\.spec.name) ?? [],
					"progress": progressState(of: tab),
					"agentStatus": store.agentStatuses[tab.id].map { "\($0)" } ?? NSNull(),
					"terminalID": NSNumber(value: tab.terminalID),
					"previews": tab.previews.map { preview in ["id": NSNumber(value: preview.id), "path": preview.path, "revision": NSNumber(value: preview.revision), "width": NSNumber(value: preview.width), "error": surfaces.previewModels[preview.id]?.error ?? NSNull()] as [String: Any] },
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
		case .agentSpace: "agentSpace"
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
			"F1": TestKey(keyCode: KeyCode.f1, plain: "\u{F704}", shifted: "\u{F704}"),
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
