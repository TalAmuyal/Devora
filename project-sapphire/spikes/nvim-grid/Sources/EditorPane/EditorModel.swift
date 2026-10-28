import Foundation
import Observation

/// The main grid (`ext_linegrid` without `ext_multigrid`, so floating windows are drawn into it).
public struct Grid: Sendable {
	public struct Cell: Sendable, Equatable {
		/// One grapheme, or "" for the right half of a double-width character.
		public var text: String
		public var hl: Int
	}

	public private(set) var width = 0
	public private(set) var height = 0
	private var cells: [Cell] = []

	public subscript(row: Int, col: Int) -> Cell {
		get { cells[row * width + col] }
		set { cells[row * width + col] = newValue }
	}

	public func rowText(_ row: Int) -> String {
		(0..<width).map { self[row, $0].text }.joined()
	}

	mutating func resize(width: Int, height: Int) {
		var resized = Grid()
		resized.width = width
		resized.height = height
		resized.cells = Array(repeating: Cell(text: " ", hl: 0), count: width * height)
		for row in 0..<min(height, self.height) {
			for col in 0..<min(width, self.width) { resized[row, col] = self[row, col] }
		}
		self = resized
	}

	mutating func clear() {
		cells = Array(repeating: Cell(text: " ", hl: 0), count: width * height)
	}

	/// `grid_scroll`: rows > 0 moves the region's content up.
	mutating func scroll(top: Int, bot: Int, left: Int, right: Int, rows: Int) {
		if rows > 0 {
			for row in top..<(bot - rows) {
				for col in left..<right { self[row, col] = self[row + rows, col] }
			}
		} else if rows < 0 {
			for row in stride(from: bot - 1, through: top - rows, by: -1) {
				for col in left..<right { self[row, col] = self[row + rows, col] }
			}
		}
	}
}

public struct CmdlineState: Sendable, Equatable {
	public var firstc: String
	public var prompt: String
	public var text: String
	/// Byte position of the cursor in `text`.
	public var pos: Int
	public var level: Int
	public var specialChar: String?
}

public struct PopupState: Sendable, Equatable {
	public var items: [PopupItem]
	public var selected: Int
	public var row: Int
	public var col: Int
}

public struct MessageLine: Sendable, Equatable, Identifiable {
	public var id: Int
	public var kind: String
	public var text: String
}

/// Everything the editor pane renders. Grid changes are pushed to the grid view (`onGridFlush`) rather than observed,
/// so typing doesn't re-evaluate SwiftUI; the overlays (cmdline, popup menu, messages) are observed.
@MainActor
@Observable
public final class EditorModel {
	public private(set) var cmdline: CmdlineState?
	public private(set) var popup: PopupState?
	public private(set) var messages: [MessageLine] = []
	public private(set) var showmode = ""
	public private(set) var modeName = "normal"

	@ObservationIgnored public private(set) var grid = Grid()
	@ObservationIgnored public private(set) var cursor = (row: 0, col: 0)
	@ObservationIgnored public private(set) var highlights: [Int: HighlightAttr] = [:]
	@ObservationIgnored public private(set) var defaultColors = (fg: UInt32(0xffffff), bg: UInt32(0), sp: UInt32(0xff0000))
	@ObservationIgnored public private(set) var cursorModes: [CursorMode] = []
	@ObservationIgnored public private(set) var modeIndex = 0
	/// Rows changed since the last flush, or nil for "everything" (resize, clear, colors).
	@ObservationIgnored private var dirtyRows: IndexSet? = IndexSet()
	@ObservationIgnored private var nextMessageID = 0
	@ObservationIgnored public var onGridFlush: ((IndexSet?) -> Void)?
	@ObservationIgnored public var onFlush: (() -> Void)?

	public init() {}

	public var cursorMode: CursorMode? { modeIndex < cursorModes.count ? cursorModes[modeIndex] : nil }

	func apply(_ events: [RedrawEvent]) {
		for event in events { apply(event) }
	}

	private func markAll() { dirtyRows = nil }
	private func mark(_ row: Int) { dirtyRows?.insert(row) }

	private func apply(_ event: RedrawEvent) {
		switch event {
		case .gridResize(_, let width, let height):
			grid.resize(width: width, height: height)
			markAll()
		case .gridLine(_, let row, let colStart, let runs):
			guard row < grid.height else { return }
			var col = colStart
			for run in runs {
				for _ in 0..<run.repeatCount where col < grid.width {
					grid[row, col] = Grid.Cell(text: run.text, hl: run.hl)
					col += 1
				}
			}
			mark(row)
		case .gridClear:
			grid.clear()
			markAll()
		case .gridCursorGoto(_, let row, let col):
			mark(cursor.row)
			cursor = (row, col)
			mark(row)
		case .gridScroll(_, let top, let bot, let left, let right, let rows):
			grid.scroll(top: top, bot: bot, left: left, right: right, rows: rows)
			dirtyRows?.insert(integersIn: top..<bot)
		case .hlAttrDefine(let id, let attr):
			highlights[id] = attr
			markAll()
		case .defaultColorsSet(let fg, let bg, let sp):
			defaultColors = (fg, bg, sp)
			markAll()
		case .modeInfoSet(_, let modes):
			cursorModes = modes
		case .modeChange(let name, let index):
			modeName = name
			modeIndex = index
			mark(cursor.row)
		case .cmdlineShow(let content, let pos, let firstc, let prompt, _, let level):
			cmdline = CmdlineState(
				firstc: firstc, prompt: prompt, text: content.map(\.text).joined(), pos: pos, level: level)
		case .cmdlinePos(let pos, _):
			cmdline?.pos = pos
			cmdline?.specialChar = nil
		case .cmdlineSpecialChar(let char, _, _):
			cmdline?.specialChar = char
		case .cmdlineHide:
			cmdline = nil
		case .popupmenuShow(let items, let selected, let row, let col):
			popup = PopupState(items: items, selected: selected, row: row, col: col)
		case .popupmenuSelect(let selected):
			popup?.selected = selected
		case .popupmenuHide:
			popup = nil
		case .msgShow(let kind, let content, let replaceLast):
			if replaceLast, !messages.isEmpty { messages.removeLast() }
			messages.append(MessageLine(id: nextMessageID, kind: kind, text: content.map(\.text).joined()))
			nextMessageID += 1
		case .msgClear:
			messages.removeAll()
		case .msgShowmode(let content):
			showmode = content.map(\.text).joined()
		case .msgShowcmd, .msgRuler:
			break
		case .flush:
			let dirty = dirtyRows
			dirtyRows = IndexSet()
			onGridFlush?(dirty)
			onFlush?()
		}
	}
}
