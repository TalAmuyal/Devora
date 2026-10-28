import CoreBindings
import EditorPane
import Foundation
import Testing

/// Drives a real `nvim --embed --clean` (no user config) through `EditorSession`, headless.
@MainActor
struct EditorTests {
	let draftsDir = FileManager.default.temporaryDirectory.appending(path: "nvim-grid-tests-\(UUID().uuidString)")

	func makeSession(draftID: String = "draft", extraArgs: [String] = []) throws -> EditorSession {
		let host = URL(filePath: #filePath).deletingLastPathComponent().appending(path: "../../host/devora_host.lua")
		let nvim = try #require(["/opt/homebrew/bin/nvim", "/usr/local/bin/nvim"].first {
			FileManager.default.isExecutableFile(atPath: $0)
		})
		let spec = NvimSpawnSpec(
			program: nvim, args: ["--embed", "--clean", "--cmd", "luafile \(host.standardizedFileURL.path)"] + extraArgs,
			env: [], envRemove: ["NVIM"])
		return EditorSession(spec: spec, drafts: try DraftStore(dir: draftsDir.path), draftID: draftID)
	}

	func eventually(_ what: Comment, timeout: Duration = .seconds(3), _ condition: () -> Bool) async throws {
		let deadline = ContinuousClock.now + timeout
		while !condition() {
			guard ContinuousClock.now < deadline else {
				Issue.record("timed out waiting for: \(what)")
				return
			}
			try await Task.sleep(for: .milliseconds(10))
		}
	}

	@Test func typedTextReachesTheGrid() async throws {
		let session = try makeSession()
		try await session.start(cols: 40, rows: 10)
		session.input("ihello wörld 👋<Esc>")
		try await eventually("grid row 0") { session.model.grid.rowText(0).hasPrefix("hello wörld 👋") }
		#expect(session.model.cursor.row == 0)
		session.kill()
	}

	@Test(arguments: [false, true])
	func startupErrorsDoNotHangTheHost(extMessages: Bool) async throws {
		let session = try makeSession(extraArgs: ["--cmd", "echoerr 'first error' | echoerr 'second error'"])
		try await session.start(cols: 40, rows: 10, options: UIOptions(messages: extMessages))
		// ext_messages hands errors to the host as msg_show, so Neovim never stops at a press-enter prompt.
		#expect(extMessages ? session.startupPrompts.isEmpty : !session.startupPrompts.isEmpty)
		session.input("ihello<Esc>")
		try await eventually("editor usable after the prompt") { session.lines == ["hello"] }
		session.kill()
	}

	@Test func cmdlineIsExternal() async throws {
		let session = try makeSession()
		try await session.start(cols: 40, rows: 10)
		session.input(":echo 42")
		try await eventually("cmdline shown") { session.model.cmdline?.text == "echo 42" }
		#expect(session.model.cmdline?.firstc == ":")
		session.input("<Esc>")
		try await eventually("cmdline hidden") { session.model.cmdline == nil }
		session.kill()
	}

	@Test func popupmenuIsExternal() async throws {
		let session = try makeSession()
		try await session.start(cols: 40, rows: 10)
		session.input("ihello helium hel<C-n>")
		try await eventually("popup menu") { session.model.popup != nil }
		#expect(Set(session.model.popup?.items.map(\.word) ?? []) == ["hello", "helium"])
		session.kill()
	}

	@Test func navigationPastTheLastWindowNotifiesTheHost() async throws {
		let session = try makeSession()
		var edges: [PaneDirection] = []
		session.onEdge = { edges.append($0) }
		try await session.start(cols: 40, rows: 10)
		try await session.command("vsplit")
		session.input("<C-l>")  // moves between Neovim's two windows
		session.input("<C-l>")  // already rightmost: hand off
		try await eventually("edge notification") { edges == [.right] }
		session.input("<C-w>h<C-w>h")
		try await eventually("edge notification") { edges == [.right, .left] }
		session.kill()
	}

	@Test func draftSurvivesKill9WithCursorAndUndo() async throws {
		let first = try makeSession()
		var saved: Draft?
		first.onDraftSaved = { saved = $0 }
		try await first.start(cols: 40, rows: 10)
		first.input("ihello<CR>world<Esc>")
		first.input("ofoo bar<Esc>b")
		try await eventually("draft saved", timeout: .seconds(5)) { saved?.lines == ["hello", "world", "foo bar"] }
		#expect(saved?.cursorRow == 3)
		#expect(saved?.cursorCol == 4)
		first.kill(signal: SIGKILL)

		let second = try makeSession()
		try await second.start(cols: 40, rows: 10)
		try await eventually("restored text") { second.model.grid.rowText(2).hasPrefix("foo bar") }
		let cursor = try await second.request("nvim_win_get_cursor", [0]).array?.compactMap(\.int)
		#expect(cursor == [3, 4])
		#expect(try await second.request("nvim_buf_get_option", [0, "modified"]).bool == false)

		second.input("u")
		try await eventually("undo removes the last change") { second.lines == ["hello", "world"] }
		second.input("u")
		try await eventually("undo reaches the first change") { second.lines == [""] }
		second.kill()
	}
}
