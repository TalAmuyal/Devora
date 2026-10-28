import AppKit
import CoreText
import QuartzCore

/// Draws the Neovim grid with Core Text and turns keyboard and IME input into `nvim_input`.
@MainActor
final class GridView: NSView, @preconcurrency NSTextInputClient {
	let session: EditorSession
	let latency: LatencyRecorder
	private let fonts: Fonts
	private var markedText = ""
	private var colorCache: [UInt32: CGColor] = [:]
	private var displayLink: CADisplayLink?
	private var lastSize = (cols: 0, rows: 0)

	var cellSize: CGSize { fonts.cell }
	var onFocusChange: ((Bool) -> Void)?

	init(session: EditorSession, latency: LatencyRecorder, fontSize: CGFloat = 13) {
		self.session = session
		self.latency = latency
		self.fonts = Fonts(size: fontSize)
		super.init(frame: .zero)
		// Since macOS 14 views don't clip by default, and draw(_:) can get a dirty rect past the bounds: the
		// background fill painted over the neighbouring pane.
		clipsToBounds = true
		session.model.onGridFlush = { [weak self] dirty in self?.gridFlushed(dirty) }
		latency.inject = { [weak self] keys in self?.send(keys) }
	}

	@available(*, unavailable)
	required init?(coder: NSCoder) { fatalError() }

	override var isFlipped: Bool { true }
	override var acceptsFirstResponder: Bool { true }
	override var isOpaque: Bool { true }

	override func becomeFirstResponder() -> Bool {
		onFocusChange?(true)
		needsDisplay = true
		return true
	}

	override func resignFirstResponder() -> Bool {
		onFocusChange?(false)
		needsDisplay = true
		return true
	}

	override func viewDidMoveToWindow() {
		super.viewDidMoveToWindow()
		displayLink?.invalidate()
		guard window != nil else { return }
		let link = displayLink(target: self, selector: #selector(frameTick(_:)))
		link.preferredFrameRateRange = CAFrameRateRange(minimum: 120, maximum: 120, preferred: 120)
		link.add(to: .main, forMode: .common)
		displayLink = link
	}

	@objc private func frameTick(_ link: CADisplayLink) {
		latency.vsync(target: link.targetTimestamp)
	}

	override func setFrameSize(_ newSize: NSSize) {
		super.setFrameSize(newSize)
		let cols = max(1, Int(newSize.width / fonts.cell.width))
		let rows = max(1, Int(newSize.height / fonts.cell.height))
		if (cols, rows) != lastSize {
			lastSize = (cols, rows)
			session.resize(cols: cols, rows: rows)
		}
	}

	var gridSize: (cols: Int, rows: Int) {
		(max(1, Int(bounds.width / fonts.cell.width)), max(1, Int(bounds.height / fonts.cell.height)))
	}

	// MARK: Rendering

	private func gridFlushed(_ dirty: IndexSet?) {
		latency.flushed()
		guard let dirty else {
			needsDisplay = true
			if drawsOnFlush { displayIfNeeded() }
			return
		}
		for row in dirty {
			setNeedsDisplay(CGRect(x: 0, y: CGFloat(row) * fonts.cell.height, width: bounds.width, height: fonts.cell.height))
		}
		let cursorRow = session.model.cursor.row
		setNeedsDisplay(CGRect(x: 0, y: CGFloat(cursorRow) * fonts.cell.height, width: bounds.width, height: fonts.cell.height))
		if drawsOnFlush { displayIfNeeded() }
	}

	/// Draw as soon as a flush arrives instead of waiting for the next update cycle (the latency bench compares both).
	var drawsOnFlush = ProcessInfo.processInfo.arguments.contains("--draw-on-flush")

	private func color(_ rgb: UInt32) -> CGColor {
		if let c = colorCache[rgb] { return c }
		let c = CGColor(
			srgbRed: CGFloat((rgb >> 16) & 0xff) / 255, green: CGFloat((rgb >> 8) & 0xff) / 255,
			blue: CGFloat(rgb & 0xff) / 255, alpha: 1)
		colorCache[rgb] = c
		return c
	}

	private func colors(_ hl: Int) -> (fg: UInt32, bg: UInt32, attr: HighlightAttr) {
		let model = session.model
		let attr = model.highlights[hl] ?? HighlightAttr()
		var fg = attr.fg ?? model.defaultColors.fg
		var bg = attr.bg ?? model.defaultColors.bg
		if attr.reverse { swap(&fg, &bg) }
		return (fg, bg, attr)
	}

	override func draw(_ dirtyRect: NSRect) {
		guard let ctx = NSGraphicsContext.current?.cgContext else { return }
		let model = session.model
		let grid = model.grid
		let cell = fonts.cell
		ctx.setFillColor(color(model.defaultColors.bg))
		ctx.fill(dirtyRect.intersection(bounds))
		ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)

		let firstRow = max(0, Int(dirtyRect.minY / cell.height))
		let lastRow = min(grid.height - 1, Int(dirtyRect.maxY / cell.height))
		guard firstRow <= lastRow else { return }

		for row in firstRow...lastRow {
			let y = CGFloat(row) * cell.height
			var col = 0
			while col < grid.width {
				let hl = grid[row, col].hl
				var end = col + 1
				while end < grid.width, grid[row, end].hl == hl { end += 1 }
				drawRun(ctx, row: row, from: col, to: end, hl: hl, y: y)
				col = end
			}
		}
		drawCursor(ctx)
		latency.drawn()
	}

	private func drawRun(_ ctx: CGContext, row: Int, from: Int, to: Int, hl: Int, y: CGFloat) {
		let grid = session.model.grid
		let cell = fonts.cell
		let (fg, bg, attr) = colors(hl)
		if bg != session.model.defaultColors.bg {
			ctx.setFillColor(color(bg))
			ctx.fill(CGRect(x: CGFloat(from) * cell.width, y: y, width: CGFloat(to - from) * cell.width, height: cell.height))
		}
		let font = fonts.font(bold: attr.bold, italic: attr.italic)
		let baseline = y + fonts.ascent
		// ASCII runs keep the font's own advances, which match the cell width; anything else is placed cell by cell so
		// fallback fonts (CJK, emoji) can't push later cells off the grid.
		var text = ""
		var runStart = from
		func flush(_ upTo: Int) {
			if !text.isEmpty {
				drawText(ctx, text, font: font, fg: fg, x: CGFloat(runStart) * cell.width, baseline: baseline)
			}
			text = ""
			runStart = upTo
		}
		for col in from..<to {
			let t = grid[row, col].text
			if t.utf8.count == 1, t != " " || !text.isEmpty {
				text += t
			} else {
				flush(col + 1)
				if !t.isEmpty, t != " " {
					drawText(ctx, t, font: font, fg: fg, x: CGFloat(col) * cell.width, baseline: baseline)
				}
				runStart = col + 1
			}
		}
		flush(to)
		if attr.underline || attr.undercurl || attr.strikethrough {
			ctx.setStrokeColor(color(attr.sp ?? fg))
			ctx.setLineWidth(1)
			let x0 = CGFloat(from) * cell.width, x1 = CGFloat(to) * cell.width
			let lineY = attr.strikethrough ? y + cell.height / 2 : baseline + 2
			ctx.move(to: CGPoint(x: x0, y: lineY))
			ctx.addLine(to: CGPoint(x: x1, y: lineY))
			ctx.strokePath()
		}
	}

	private func drawText(_ ctx: CGContext, _ text: String, font: CTFont, fg: UInt32, x: CGFloat, baseline: CGFloat) {
		let attributed = NSAttributedString(string: text, attributes: [
			NSAttributedString.Key(kCTFontAttributeName as String): font,
			NSAttributedString.Key(kCTForegroundColorAttributeName as String): color(fg),
		])
		let line = CTLineCreateWithAttributedString(attributed)
		ctx.textPosition = CGPoint(x: x, y: baseline)
		CTLineDraw(line, ctx)
	}

	private func drawCursor(_ ctx: CGContext) {
		let model = session.model
		let (row, col) = model.cursor
		guard row < model.grid.height, col < model.grid.width else { return }
		let cell = fonts.cell
		let rect = CGRect(x: CGFloat(col) * cell.width, y: CGFloat(row) * cell.height, width: cell.width, height: cell.height)
		let (fg, bg, _) = colors(model.grid[row, col].hl)

		if !markedText.isEmpty {
			// IME preedit is drawn natively at the cursor, underlined, until the input method commits it.
			let width = CGFloat(max(1, markedText.count)) * cell.width * 2
			ctx.setFillColor(color(bg))
			ctx.fill(CGRect(x: rect.minX, y: rect.minY, width: width, height: cell.height))
			drawText(ctx, markedText, font: fonts.font(bold: false, italic: false), fg: fg, x: rect.minX, baseline: rect.minY + fonts.ascent)
			ctx.setStrokeColor(color(fg))
			ctx.move(to: CGPoint(x: rect.minX, y: rect.maxY - 1))
			ctx.addLine(to: CGPoint(x: rect.minX + width, y: rect.maxY - 1))
			ctx.strokePath()
			return
		}

		let focused = window?.firstResponder === self
		ctx.setFillColor(color(fg))
		switch model.cursorMode?.shape ?? .block {
		case .block where focused:
			ctx.fill(rect)
			let t = model.grid[row, col].text
			if !t.isEmpty, t != " " {
				drawText(ctx, t, font: fonts.font(bold: false, italic: false), fg: bg, x: rect.minX, baseline: rect.minY + fonts.ascent)
			}
		case .block:
			ctx.setStrokeColor(color(fg))
			ctx.stroke(rect.insetBy(dx: 0.5, dy: 0.5))
		case .vertical:
			let w = max(1, cell.width * CGFloat(model.cursorMode?.cellPercentage ?? 25) / 100)
			ctx.fill(CGRect(x: rect.minX, y: rect.minY, width: w, height: cell.height))
		case .horizontal:
			let h = max(1, cell.height * CGFloat(model.cursorMode?.cellPercentage ?? 20) / 100)
			ctx.fill(CGRect(x: rect.minX, y: rect.maxY - h, width: cell.width, height: h))
		}
	}

	// MARK: Input

	/// The single path from the host to Neovim's input queue, used by keyDown and by the scripted latency bench.
	func send(_ keys: String) {
		latency.keySent()
		session.input(keys)
	}

	override func keyDown(with event: NSEvent) {
		latency.keyEventAge(event)
		if markedText.isEmpty, let keys = KeyTranslator.special(event) {
			send(keys)
		} else {
			interpretKeyEvents([event])
		}
	}

	func insertText(_ string: Any, replacementRange: NSRange) {
		let text = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
		markedText = ""
		send(text.replacingOccurrences(of: "<", with: "<lt>"))
		needsDisplay = true
	}

	override func doCommand(by selector: Selector) {
		// Keys that interpretKeyEvents maps to editing commands but KeyTranslator didn't claim.
		let keys: String? = switch selector {
		case #selector(insertNewline(_:)): "<CR>"
		case #selector(insertTab(_:)): "<Tab>"
		case #selector(deleteBackward(_:)): "<BS>"
		case #selector(cancelOperation(_:)): "<Esc>"
		default: nil
		}
		if let keys { send(keys) }
	}

	func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
		markedText = (string as? NSAttributedString)?.string ?? (string as? String) ?? ""
		needsDisplay = true
	}

	func unmarkText() {
		markedText = ""
		needsDisplay = true
	}

	func selectedRange() -> NSRange { NSRange(location: 0, length: 0) }
	func markedRange() -> NSRange { markedText.isEmpty ? NSRange(location: NSNotFound, length: 0) : NSRange(location: 0, length: markedText.utf16.count) }
	func hasMarkedText() -> Bool { !markedText.isEmpty }
	func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? { nil }
	func validAttributesForMarkedText() -> [NSAttributedString.Key] { [] }
	func characterIndex(for point: NSPoint) -> Int { NSNotFound }

	/// Where the input method's candidate window goes: under the Neovim cursor.
	func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
		let (row, col) = session.model.cursor
		let local = CGRect(
			x: CGFloat(col) * fonts.cell.width, y: CGFloat(row) * fonts.cell.height,
			width: fonts.cell.width, height: fonts.cell.height)
		guard let window else { return local }
		return window.convertToScreen(convert(local, to: nil))
	}
}

/// Monospaced fonts and the cell size derived from them.
@MainActor
struct Fonts {
	let regular: CTFont
	let bold: CTFont
	let italic: CTFont
	let boldItalic: CTFont
	let cell: CGSize
	let ascent: CGFloat

	init(size: CGFloat) {
		let base = NSFont.monospacedSystemFont(ofSize: size, weight: .regular) as CTFont
		func variant(_ traits: CTFontSymbolicTraits) -> CTFont {
			CTFontCreateCopyWithSymbolicTraits(base, size, nil, traits, traits) ?? base
		}
		regular = base
		bold = variant(.traitBold)
		italic = variant(.traitItalic)
		boldItalic = variant([.traitBold, .traitItalic])
		var glyph = CGGlyph()
		var advance = CGSize.zero
		var m: UniChar = 0x4D  // "M"
		CTFontGetGlyphsForCharacters(base, &m, &glyph, 1)
		CTFontGetAdvancesForGlyphs(base, .horizontal, &glyph, &advance, 1)
		ascent = ceil(CTFontGetAscent(base))
		cell = CGSize(
			width: advance.width,
			height: ceil(CTFontGetAscent(base) + CTFontGetDescent(base) + CTFontGetLeading(base)) + 2)
	}

	func font(bold isBold: Bool, italic isItalic: Bool) -> CTFont {
		switch (isBold, isItalic) {
		case (true, true): boldItalic
		case (true, false): bold
		case (false, true): italic
		case (false, false): regular
		}
	}
}

/// Keys that bypass the input method and go straight to Neovim in its key notation.
enum KeyTranslator {
	static func special(_ event: NSEvent) -> String? {
		let flags = event.modifierFlags.intersection([.control, .option, .shift, .command])
		if flags.contains(.command) { return nil }
		let named: String? = switch Int(event.keyCode) {
		case 0x24, 0x4c: "CR"
		case 0x30: "Tab"
		case 0x33: "BS"
		case 0x35: "Esc"
		case 0x75: "Del"
		case 0x7b: "Left"
		case 0x7c: "Right"
		case 0x7d: "Down"
		case 0x7e: "Up"
		case 0x73: "Home"
		case 0x77: "End"
		case 0x74: "PageUp"
		case 0x79: "PageDown"
		case 0x7a: "F1"
		case 0x78: "F2"
		case 0x63: "F3"
		case 0x76: "F4"
		default: nil
		}
		var mods = ""
		if flags.contains(.control) { mods += "C-" }
		if let named {
			if flags.contains(.shift) { mods += "S-" }
			if flags.contains(.option) { mods += "M-" }
			return "<\(mods)\(named)>"
		}
		// Control chords skip the input method; plain and Option keys go through it (dead keys, IME, emoji).
		guard flags.contains(.control), let base = event.charactersIgnoringModifiers?.lowercased(), !base.isEmpty else {
			return nil
		}
		let key = base == "<" ? "lt" : base
		return "<\(mods)\(key)>"
	}
}
