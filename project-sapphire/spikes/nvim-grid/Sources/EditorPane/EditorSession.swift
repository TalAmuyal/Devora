import CoreBindings
import Foundation
import MsgPack
import NvimRPC
import Synchronization

public enum PaneDirection: String, Sendable {
	case left = "h", down = "j", up = "k", right = "l"

	public var opposite: PaneDirection {
		switch self {
		case .left: .right
		case .right: .left
		case .up: .down
		case .down: .up
		}
	}
}

public struct UIOptions: Sendable {
	public var cmdline = true
	public var popupmenu = true
	public var messages = false

	public init(cmdline: Bool = true, popupmenu: Bool = true, messages: Bool = false) {
		self.cmdline = cmdline
		self.popupmenu = popupmenu
		self.messages = messages
	}
}

/// One embedded Neovim: the process, the UI attachment, the draft it edits, and the pane handoff.
@MainActor
public final class EditorSession {
	public let model = EditorModel()
	/// Neovim has no window in this direction: the host should move focus to its neighbouring pane.
	public var onEdge: ((PaneDirection) -> Void)?
	public var onDraftSaved: ((Draft) -> Void)?
	public private(set) var lines: [String] = []
	/// Modes (`nvim_get_mode`) of the blocking prompts the host dismissed during startup: a sign the user's config
	/// errored or asked for input when embedded.
	public private(set) var startupPrompts: [String] = []
	public var pid: Int32? { client?.pid }

	private let spec: NvimSpawnSpec
	private let drafts: DraftStore
	private let draftID: String
	private let debounce: Duration
	private var client: NvimClient?
	private var changedtick: UInt64 = 0
	private var saveTask: Task<Void, Never>?

	public init(spec: NvimSpawnSpec, drafts: DraftStore, draftID: String, debounce: Duration = .milliseconds(250)) {
		self.spec = spec
		self.drafts = drafts
		self.draftID = draftID
		self.debounce = debounce
	}

	public func start(cols: Int, rows: Int, options: UIOptions = UIOptions()) async throws {
		let pending = Mutex<[RedrawEvent]>([])
		let client = try NvimClient(
			program: spec.program, arguments: spec.args,
			environment: Dictionary(spec.env.map { ($0.key, $0.value) }, uniquingKeysWith: { $1 }),
			removing: spec.envRemove,
			onExit: { [weak self] status in
				DispatchQueue.main.async { MainActor.assumeIsolated { self?.exited(status: status) } }
			}
		) { [weak self] method, params in
			switch method {
			case "redraw":
				// Batch on the reader thread and hop to the main actor once per flush. DispatchQueue.main keeps the
				// batches in order, which per-call `Task { @MainActor in }` does not guarantee.
				var parsed: [RedrawEvent] = []
				RedrawParser.parse(params, into: &parsed)
				let batch: [RedrawEvent]? = pending.withLock { buffer in
					buffer.append(contentsOf: parsed)
					guard parsed.contains(where: { if case .flush = $0 { true } else { false } }) else { return nil }
					defer { buffer.removeAll(keepingCapacity: true) }
					return buffer
				}
				if let batch {
					DispatchQueue.main.async { MainActor.assumeIsolated { self?.model.apply(batch) } }
				}
			case "nvim_buf_lines_event":
				DispatchQueue.main.async { MainActor.assumeIsolated { self?.applyLinesEvent(params) } }
			case "devora_ready":
				DispatchQueue.main.async { MainActor.assumeIsolated { self?.markReady() } }
			case "devora_edge":
				let direction = params.first?.string.flatMap(PaneDirection.init(rawValue:))
				DispatchQueue.main.async {
					MainActor.assumeIsolated { if let direction { self?.onEdge?(direction) } }
				}
			default:
				break
			}
		}
		self.client = client

		var opts: [MsgPack.Pair] = [.init("rgb", true), .init("ext_linegrid", true)]
		if options.cmdline { opts.append(.init("ext_cmdline", true)) }
		if options.popupmenu { opts.append(.init("ext_popupmenu", true)) }
		if options.messages { opts.append(.init("ext_messages", true)) }
		_ = try await client.request("nvim_ui_attach", [.int(Int64(cols)), .int(Int64(rows)), .map(opts)])

		try await waitUntilReady()
		_ = try await client.request("nvim_exec_lua", ["DevoraHost.setup_draft_buffer()", []])
		if let draft = try drafts.load(id: draftID) {
			_ = try await client.request("nvim_exec_lua", [
				"DevoraHost.restore(...)",
				[.array(draft.lines.map { .string($0) }), .uint(UInt64(draft.cursorRow)), .uint(UInt64(draft.cursorCol)),
				 .string(try drafts.undoFile(id: draftID))],
			])
		}
		// send_buffer: true gives the current text as the first lines event, so `lines` starts in sync.
		_ = try await client.request("nvim_buf_attach", [.int(0), true, .map([])])
	}

	private var isReady = false
	private var exitStatus: Int32?
	private var readyWaiters: [CheckedContinuation<Void, Error>] = []
	/// Neovim exited on its own (not through `shutdown`/`kill`).
	public var onExit: ((Int32) -> Void)?

	private func markReady() {
		isReady = true
		for waiter in readyWaiters { waiter.resume() }
		readyWaiters.removeAll()
	}

	private func exited(status: Int32) {
		exitStatus = status
		for waiter in readyWaiters { waiter.resume(throwing: NvimRPCError(method: "start", payload: .string("nvim exited with status \(status) during startup"))) }
		readyWaiters.removeAll()
		if client != nil { onExit?(status) }
	}

	/// Waits for the host plugin's `devora_ready` (VimEnter: the user's config has finished). Anything in that config
	/// that waits for a key (a press-enter or `-- More --` prompt, `confirm()`) would stall startup forever, so
	/// meanwhile poll `nvim_get_mode`, a fast request Neovim answers even then, and dismiss blocking prompts.
	private func waitUntilReady() async throws {
		guard let client else { throw NvimRPCError(method: "start", payload: "not started") }
		let watchdog = Task { [weak self] in
			while !Task.isCancelled {
				try? await Task.sleep(for: .milliseconds(100))
				guard let mode = try? await client.request("nvim_get_mode"), mode["blocking"]?.bool == true else { continue }
				self?.startupPrompts.append(mode["mode"]?.string ?? "?")
				client.notify("nvim_input", ["<CR>"])
			}
		}
		defer { watchdog.cancel() }
		if let exitStatus { throw NvimRPCError(method: "start", payload: .string("nvim exited with status \(exitStatus)")) }
		if !isReady {
			try await withCheckedThrowingContinuation { readyWaiters.append($0) }
		}
	}

	/// Keys in Neovim notation (`<C-x>`, `<lt>` …). A notification, so typing never waits on a round trip.
	public func input(_ keys: String) {
		client?.notify("nvim_input", [.string(keys)])
	}

	public func resize(cols: Int, rows: Int) {
		client?.notify("nvim_ui_try_resize", [.int(Int64(cols)), .int(Int64(rows))])
	}

	public func command(_ command: String) async throws {
		_ = try await client?.request("nvim_command", [.string(command)])
	}

	public func request(_ method: String, _ params: [MsgPack] = []) async throws -> MsgPack {
		guard let client else { throw NvimRPCError(method: method, payload: "not started") }
		return try await client.request(method, params)
	}

	/// Focus arrives from the host pane in `direction` (e.g. from the left after `Ctrl-l`): enter the nearest window.
	public func enter(from direction: PaneDirection) {
		client?.notify("nvim_command", [.string("999wincmd \(direction.rawValue)")])
	}

	public func shutdown() async {
		await client?.shutdown()
		client = nil
	}

	public func kill(signal: Int32 = SIGKILL) {
		client?.kill(signal: signal)
		client?.waitUntilExit()
		client = nil
	}

	private func applyLinesEvent(_ params: [MsgPack]) {
		// [buf, changedtick, firstline, lastline, linedata, more]
		guard params.count >= 5, let first = params[2].int, let last = params[3].int else { return }
		let data = (params[4].array ?? []).map { $0.string ?? "" }
		if let tick = params[1].int { changedtick = UInt64(tick) }
		let end = last == -1 ? lines.count : min(last, lines.count)
		lines.replaceSubrange(min(first, lines.count)..<end, with: data)
		scheduleSave()
	}

	private func scheduleSave() {
		saveTask?.cancel()
		saveTask = Task { [weak self, debounce] in
			try? await Task.sleep(for: debounce)
			guard !Task.isCancelled else { return }
			await self?.saveDraft()
		}
	}

	private func saveDraft() async {
		guard let client else { return }
		do {
			let cursor = try await client.request("nvim_win_get_cursor", [.int(0)]).array ?? []
			_ = try await client.request("nvim_exec_lua", ["DevoraHost.save_undo(...)", [.string(try drafts.undoFile(id: draftID))]])
			let draft = Draft(
				id: draftID, lines: lines, cursorRow: UInt32(cursor.first?.int ?? 1),
				cursorCol: UInt32(cursor.last?.int ?? 0), changedtick: changedtick)
			try drafts.save(draft: draft)
			onDraftSaved?(draft)
		} catch {
			// A spike: a failed save is visible as a missing onDraftSaved, and the restore test catches it.
		}
	}
}
