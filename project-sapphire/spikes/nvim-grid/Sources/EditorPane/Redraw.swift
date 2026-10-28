import MsgPack

/// Typed `redraw` events (`:help ui-linegrid`, `ui-cmdline`, `ui-popupmenu`, `ui-messages`), parsed off the main thread.
enum RedrawEvent: Sendable {
	case gridResize(grid: Int, width: Int, height: Int)
	case gridLine(grid: Int, row: Int, colStart: Int, cells: [CellRun])
	case gridClear(grid: Int)
	case gridCursorGoto(grid: Int, row: Int, col: Int)
	case gridScroll(grid: Int, top: Int, bot: Int, left: Int, right: Int, rows: Int)
	case hlAttrDefine(id: Int, attr: HighlightAttr)
	case defaultColorsSet(fg: UInt32, bg: UInt32, sp: UInt32)
	case modeInfoSet(cursorStyleEnabled: Bool, modes: [CursorMode])
	case modeChange(name: String, index: Int)
	case cmdlineShow(content: [StyledChunk], pos: Int, firstc: String, prompt: String, indent: Int, level: Int)
	case cmdlinePos(pos: Int, level: Int)
	case cmdlineSpecialChar(char: String, shift: Bool, level: Int)
	case cmdlineHide(level: Int)
	case popupmenuShow(items: [PopupItem], selected: Int, row: Int, col: Int)
	case popupmenuSelect(selected: Int)
	case popupmenuHide
	case msgShow(kind: String, content: [StyledChunk], replaceLast: Bool)
	case msgClear
	case msgShowmode(content: [StyledChunk])
	case msgShowcmd(content: [StyledChunk])
	case msgRuler(content: [StyledChunk])
	case flush
}

/// One `grid_line` cell entry: `text` repeated `repeatCount` times with highlight `hl`.
struct CellRun: Sendable {
	var text: String
	var hl: Int
	var repeatCount: Int
}

struct StyledChunk: Sendable, Equatable {
	var hl: Int
	var text: String
}

public struct PopupItem: Sendable, Equatable, Identifiable {
	public var id: Int
	public var word: String
	public var kind: String
	public var menu: String
}

public struct HighlightAttr: Sendable, Equatable {
	public var fg: UInt32?
	public var bg: UInt32?
	public var sp: UInt32?
	public var bold = false
	public var italic = false
	public var underline = false
	public var undercurl = false
	public var strikethrough = false
	public var reverse = false
}

public struct CursorMode: Sendable, Equatable {
	public enum Shape: Sendable { case block, horizontal, vertical }
	public var name: String
	public var shape: Shape
	/// Percentage of the cell the cursor covers for `horizontal` and `vertical`.
	public var cellPercentage: Int
}

enum RedrawParser {
	/// `params` of one `redraw` notification: `[[name, args...], [name, args...], ...]`.
	static func parse(_ params: [MsgPack], into events: inout [RedrawEvent]) {
		for update in params {
			guard let items = update.array, let name = items.first?.string else { continue }
			for case .array(let args) in items.dropFirst() {
				if let event = event(name, args) { events.append(event) }
			}
		}
	}

	private static func event(_ name: String, _ a: [MsgPack]) -> RedrawEvent? {
		func int(_ i: Int) -> Int { i < a.count ? a[i].int ?? 0 : 0 }
		func str(_ i: Int) -> String { i < a.count ? a[i].string ?? "" : "" }
		switch name {
		case "grid_resize":
			return .gridResize(grid: int(0), width: int(1), height: int(2))
		case "grid_line":
			var runs: [CellRun] = []
			var hl = 0
			for case .array(let cell) in a.count > 3 ? a[3].array ?? [] : [] {
				if cell.count > 1, let id = cell[1].int { hl = id }
				let count = cell.count > 2 ? cell[2].int ?? 1 : 1
				runs.append(CellRun(text: cell.first?.string ?? " ", hl: hl, repeatCount: count))
			}
			return .gridLine(grid: int(0), row: int(1), colStart: int(2), cells: runs)
		case "grid_clear":
			return .gridClear(grid: int(0))
		case "grid_cursor_goto":
			return .gridCursorGoto(grid: int(0), row: int(1), col: int(2))
		case "grid_scroll":
			return .gridScroll(grid: int(0), top: int(1), bot: int(2), left: int(3), right: int(4), rows: int(5))
		case "hl_attr_define":
			let rgb = a.count > 1 ? a[1] : .nil
			func color(_ key: String) -> UInt32? { rgb[key]?.int.map { UInt32(truncatingIfNeeded: $0) } }
			func flag(_ key: String) -> Bool { rgb[key]?.bool ?? false }
			return .hlAttrDefine(id: int(0), attr: HighlightAttr(
				fg: color("foreground"), bg: color("background"), sp: color("special"),
				bold: flag("bold"), italic: flag("italic"), underline: flag("underline"), undercurl: flag("undercurl"),
				strikethrough: flag("strikethrough"), reverse: flag("reverse")))
		case "default_colors_set":
			func rgb(_ i: Int, _ fallback: UInt32) -> UInt32 {
				let v = int(i)
				return v < 0 ? fallback : UInt32(truncatingIfNeeded: v)
			}
			return .defaultColorsSet(fg: rgb(0, 0xffffff), bg: rgb(1, 0x000000), sp: rgb(2, 0xff0000))
		case "mode_info_set":
			let modes = (a.count > 1 ? a[1].array ?? [] : []).map { info in
				let shape: CursorMode.Shape = switch info["cursor_shape"]?.string {
				case "horizontal": .horizontal
				case "vertical": .vertical
				default: .block
				}
				return CursorMode(
					name: info["name"]?.string ?? "", shape: shape, cellPercentage: info["cell_percentage"]?.int ?? 100)
			}
			return .modeInfoSet(cursorStyleEnabled: a.first?.bool ?? true, modes: modes)
		case "mode_change":
			return .modeChange(name: str(0), index: int(1))
		case "cmdline_show":
			return .cmdlineShow(
				content: chunks(a.first), pos: int(1), firstc: str(2), prompt: str(3), indent: int(4), level: int(5))
		case "cmdline_pos":
			return .cmdlinePos(pos: int(0), level: int(1))
		case "cmdline_special_char":
			return .cmdlineSpecialChar(char: str(0), shift: a.count > 1 ? a[1].bool ?? false : false, level: int(2))
		case "cmdline_hide":
			return .cmdlineHide(level: int(0))
		case "popupmenu_show":
			let items = (a.first?.array ?? []).enumerated().map { i, item in
				let f = item.array ?? []
				func s(_ j: Int) -> String { j < f.count ? f[j].string ?? "" : "" }
				return PopupItem(id: i, word: s(0), kind: s(1), menu: s(2))
			}
			return .popupmenuShow(items: items, selected: int(1), row: int(2), col: int(3))
		case "popupmenu_select":
			return .popupmenuSelect(selected: int(0))
		case "popupmenu_hide":
			return .popupmenuHide
		case "msg_show":
			return .msgShow(kind: str(0), content: chunks(a.count > 1 ? a[1] : nil), replaceLast: a.count > 2 ? a[2].bool ?? false : false)
		case "msg_clear":
			return .msgClear
		case "msg_showmode":
			return .msgShowmode(content: chunks(a.first))
		case "msg_showcmd":
			return .msgShowcmd(content: chunks(a.first))
		case "msg_ruler":
			return .msgRuler(content: chunks(a.first))
		case "flush":
			return .flush
		default:
			return nil
		}
	}

	private static func chunks(_ value: MsgPack?) -> [StyledChunk] {
		(value?.array ?? []).compactMap { chunk in
			guard let parts = chunk.array, parts.count >= 2 else { return nil }
			// Neovim 0.10+ sends [attr_id, text, hl_id]; the attr id indexes hl_attr_define.
			return StyledChunk(hl: parts[0].int ?? 0, text: parts[1].string ?? "")
		}
	}
}
