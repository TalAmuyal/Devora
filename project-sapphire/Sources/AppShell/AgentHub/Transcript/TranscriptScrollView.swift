import AppKit
import CoreBindings
import Store
import SwiftUI

/// The native agent transcript (ADR-014): an `NSTableView` of hosted SwiftUI rows, fed `AgentChat`'s items.
///
/// It follows the bottom while the reader is there, keeps the visible row in place otherwise (through streaming, older
/// pages and every height correction), asks for an older page near the top, and reports where the reader is, as the
/// web transcript does. Row heights are estimated until measured (`RowHeights`): the visible rows are measured at once,
/// and the rest a few milliseconds per frame, nearest first, until every row is exact; then the display link stops.
@MainActor
final class TranscriptScrollView: NSScrollView, NSTableViewDataSource, NSTableViewDelegate {
	/// Distance from the bottom within which the reader is following it, and from the top that asks for older history.
	static let followSlack: CGFloat = 24
	static let olderSlack: CGFloat = 80
	/// The idle measurement's budget per frame.
	static let measureBudget: CFTimeInterval = 0.002

	let table = NSTableView()
	/// The rows: the chat's items, with a card after each question's tool row (`TranscriptEntry.layout`).
	private(set) var entries: [TranscriptEntry] = []
	var items: [TranscriptItem] { entries.compactMap(\.item) }
	var hasOlder = false
	/// Where the reader is, 250 ms after a scroll: the top row and its offset, or nil at the bottom.
	var onScroll: (ChatScrollAnchor?) -> Void = { _ in }
	var onNeedsOlder: () -> Void = {}
	/// Answers a card's question: its request id, the action ("allow", "deny", "always") and the card's generation.
	var onAnswer: (String, String, UInt64) -> Void = { _, _, _ in }

	private var heights = RowHeights()
	private var documents: [UInt64: (text: String, document: MarkdownDocument)] = [:]
	/// The thinking and tool rows the reader expanded, by sequence: kept through updates, cell reuse and older pages.
	private var expanded: Set<UInt64> = []
	private var pending = IndexSet()
	private let measurer = NSHostingView(rootView: AnyView(EmptyView()))
	private var idle: CADisplayLink?
	private var following = true
	/// Set while the view moves itself, so that only the reader's scrolls count as theirs.
	private var adjusting = false
	private var restoring: ChatScrollAnchor?
	private var olderRequested = false
	private var scrollReport: Task<Void, Never>?
	private var laidOutWidth: CGFloat = 0
	private var lastTop: CGFloat = 0
	private var wheelRouting = WheelRouting()
	private var wheelMonitor: Any?
	/// Draws the rows' Mermaid diagrams; its theme is the web renderer's, as the web transcript draws them.
	private let renderer: MermaidRenderer
	var diagramTheme: [String: String] = [:] {
		didSet {
			guard diagramTheme != oldValue, !entries.isEmpty else { return }
			// Every diagram is drawn again in the new colors: their rows' heights are estimates until then.
			findPending()
			measureVisible()
			scheduleIdle()
		}
	}
	private var diagramToken: UUID?
	private(set) var measurements = 0

	init(anchor: ChatScrollAnchor?, renderer: MermaidRenderer = .shared) {
		self.renderer = renderer
		restoring = anchor
		following = anchor == nil
		super.init(frame: .zero)
		measurer.safeAreaRegions = []
		diagramToken = renderer.observe { [weak self] key in self?.diagramDrawn(key) }
		let column = NSTableColumn(identifier: .init("row"))
		column.resizingMask = .autoresizingMask
		table.addTableColumn(column)
		table.headerView = nil
		table.style = .plain
		table.selectionHighlightStyle = .none
		table.intercellSpacing = .zero
		table.backgroundColor = .clear
		table.usesAutomaticRowHeights = false
		table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
		table.dataSource = self
		table.delegate = self
		table.setAccessibilityLabel("Agent transcript")
		table.setAccessibilityIdentifier("agent-transcript")
		documentView = table
		setAccessibilityIdentifier("agent-transcript-scroll")
		hasVerticalScroller = true
		drawsBackground = false
		contentView.postsBoundsChangedNotifications = true
		contentView.postsFrameChangedNotifications = true
		NotificationCenter.default.addObserver(
			self, selector: #selector(boundsChanged), name: NSView.boundsDidChangeNotification, object: contentView)
		NotificationCenter.default.addObserver(
			self, selector: #selector(frameChanged), name: NSView.frameDidChangeNotification, object: contentView)
	}

	required init?(coder: NSCoder) { fatalError("not used") }

	isolated deinit {
		if let diagramToken { renderer.stopObserving(diagramToken) }
		idle?.invalidate()
		scrollReport?.cancel()
		if let wheelMonitor { NSEvent.removeMonitor(wheelMonitor) }
	}

	var width: CGFloat { max(200, contentView.bounds.width) }

	var distanceFromBottom: CGFloat { max(0, table.bounds.height - contentView.bounds.maxY) }

	/// The first item row whose bottom is below the visible area's top, and how far its top is from that top. A card on
	/// top counts as the item row above it, since a saved position is an item's: the card is gone when it comes back.
	var visibleAnchor: ChatScrollAnchor? {
		guard let top = visibleRow else { return nil }
		let row = (0...top).reversed().first { entries[$0].item != nil } ?? entries.indices.first { entries[$0].item != nil }
		guard let row, let item = entries[row].item else { return nil }
		return ChatScrollAnchor(sequence: item.sequence, offset: Int32((table.rect(ofRow: row).minY - contentView.bounds.minY).rounded()))
	}

	/// A row kept in place across an update, with its exact offset: a rounded one would drift with every correction.
	private struct Pin {
		let key: TranscriptEntry.Key
		let offset: CGFloat
	}

	private func pin(_ row: Int) -> Pin {
		Pin(key: entries[row].key, offset: table.rect(ofRow: row).minY - contentView.bounds.minY)
	}

	private var visibleRow: Int? {
		let row = table.row(at: NSPoint(x: 1, y: contentView.bounds.minY + 0.5))
		return row >= 0 && row < entries.count ? row : nil
	}

	private var visiblePin: Pin? { visibleRow.map(pin) }

	/// Whether every row has its measured height (the idle measurement has nothing left to do).
	var isSettled: Bool { pending.isEmpty && idle == nil }

	// MARK: Updates

	/// Shows `items`, with a card for each of `questions`, whose answers carry `generation`.
	func apply(_ items: [TranscriptItem], questions: [PermissionQuestion] = [], generation: UInt64 = 0) {
		let new = TranscriptEntry.layout(items, questions: questions, generation: generation)
		let change = TranscriptChange(from: entries, to: new)
		guard change != .none else { return }
		let anchor = following ? nil : visiblePin
		let firstLoad = entries.isEmpty
		entries = new
		let sequences = Set(items.map(\.sequence))
		documents = documents.filter { sequences.contains($0.key) }
		adjusting = true
		defer { adjusting = false }
		switch change {
		case .none: return
		case .prepended(let count):
			olderRequested = false
			table.insertRows(at: IndexSet(0..<count), withAnimation: [])
		case .updated(let changed, let removedFromTop, let appended, let removedFromBottom):
			let oldCount = table.numberOfRows
			table.beginUpdates()
			if removedFromBottom > 0 {
				table.removeRows(at: IndexSet((oldCount - removedFromBottom)..<oldCount), withAnimation: [])
			}
			if removedFromTop > 0 { table.removeRows(at: IndexSet(0..<removedFromTop), withAnimation: []) }
			if !changed.isEmpty {
				let rows = IndexSet(changed)
				withoutAnimation { table.noteHeightOfRows(withIndexesChanged: rows) }
				table.reloadData(forRowIndexes: rows, columnIndexes: [0])
			}
			if appended > 0 { table.insertRows(at: IndexSet((new.count - appended)..<new.count), withAnimation: []) }
			table.endUpdates()
		case .reloaded:
			table.reloadData()
		}
		heights.keep(only: Set(new.map(\.key)))
		findPending()
		var kept: Pin?
		if firstLoad, let saved = restoring {
			// The saved position, once: at its row if the transcript has it, otherwise at the bottom, as the web one does.
			// Its row stays put while the rows around it are measured.
			restoring = nil
			following = !sequences.contains(saved.sequence)
			kept = following ? nil : Pin(key: .item(saved.sequence), offset: CGFloat(saved.offset))
			settle(kept)
		}
		measureVisible(keeping: kept)
		settle(anchor)
		scheduleIdle()
	}

	// MARK: Heights

	func numberOfRows(in tableView: NSTableView) -> Int { entries.count }

	func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
		heights.height(entries[row], expanded: isExpanded(row), width: width)
	}

	func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
		let identifier = NSUserInterfaceItemIdentifier("transcript-row")
		let cell = tableView.makeView(withIdentifier: identifier, owner: nil) as? TranscriptCell ?? TranscriptCell(identifier: identifier)
		cell.show(view(row))
		return cell
	}

	/// What row `row` draws, with its actions bound to this view.
	private func view(_ row: Int) -> TranscriptEntryView {
		switch entries[row] {
		case .item(let item):
			let sequence = item.sequence
			return TranscriptEntryView(model: .row(model(row)), toggle: { [weak self] in self?.toggle(sequence) })
		case .card(let card):
			let id = card.question.requestId
			return TranscriptEntryView(model: .card(card), answer: { [weak self] action in self?.onAnswer(id, action, card.generation) })
		}
	}

	func tableView(_ tableView: NSTableView, didRemove rowView: NSTableRowView, forRow row: Int) {
		// A row that scrolled away keeps its cell for reuse; until it shows another row, it shows (and exposes) nothing.
		(rowView.view(atColumn: 0) as? TranscriptCell)?.clear()
	}

	func tableView(_ tableView: NSTableView, shouldSelectRow row: Int) -> Bool { false }

	private func isExpanded(_ row: Int) -> Bool { entries[row].item.map { expanded.contains($0.sequence) } ?? false }

	/// An item row's model (`row` must be an item's).
	private func model(_ row: Int) -> TranscriptRowModel {
		guard let item = entries[row].item else { preconditionFailure("row \(row) is a card") }
		let expanded = isExpanded(row)
		// A collapsed thought shows only its summary, so it isn't parsed until it opens.
		guard let text = TranscriptRowModel.markdown(of: item.content), expanded || RowHeights.Kind(item) != .thinking else {
			return TranscriptRowModel(item, document: nil, expanded: expanded)
		}
		let document: MarkdownDocument
		if let cached = documents[item.sequence], cached.text == text {
			document = cached.document
		} else {
			document = MarkdownDocument(text)
			documents[item.sequence] = (text, document)
		}
		return TranscriptRowModel(item, document: document, expanded: expanded, diagrams: diagrams(of: item, in: document, requesting: true))
	}

	// MARK: Diagrams

	/// The width a diagram is drawn at: the row's, inside its margins and kind marker (`TranscriptRow`, `KindMarker`).
	private var diagramWidth: Int { Int(width - 54) }

	private func diagramKey(_ source: String) -> DiagramKey {
		DiagramKey(source: source, width: diagramWidth, scale: window?.backingScaleFactor ?? 2, theme: diagramTheme)
	}

	/// The outcome of each of `document`'s diagrams, asking the renderer for those it hasn't drawn; nil while the item
	/// streams, since a diagram half written can't be drawn (the web transcript shows its source too).
	private func diagrams(of item: TranscriptItem, in document: MarkdownDocument, requesting: Bool) -> [String: DiagramOutcome]? {
		let sources = document.diagrams
		guard !sources.isEmpty, item.state != .streaming else { return nil }
		var outcomes: [String: DiagramOutcome] = [:]
		for source in sources {
			let key = diagramKey(source)
			if let outcome = renderer.outcome(key) {
				outcomes[source] = outcome
			} else {
				// A live resize asks for a drawing at every width it passes; it waits for the end of the resize instead.
				if requesting, !inLiveResize { renderer.request(key, theme: diagramTheme) }
				outcomes[source] = .pending
			}
		}
		return outcomes
	}

	/// A fingerprint of what the row's diagrams show, so a measured height is exact only for them; 0 for a row without
	/// any, or not parsed yet (its measurement parses it).
	private func diagramPrint(_ row: Int) -> Int {
		guard let item = entries[row].item, let document = documents[item.sequence]?.document,
			let outcomes = diagrams(of: item, in: document, requesting: false)
		else { return 0 }
		var hasher = Hasher()
		for source in document.diagrams {
			switch outcomes[source] {
			case .drawn(let drawn): hasher.combine(ObjectIdentifier(drawn.image))
			case .failed: hasher.combine(1)
			case .pending, nil: hasher.combine(0)
			}
		}
		return hasher.finalize()
	}

	/// A diagram the renderer finished: the rows showing it are measured again, the visible ones at once, as any height
	/// correction, keeping the bottom or the rows the reader already saw in place.
	private func diagramDrawn(_ key: DiagramKey) {
		guard key.width == diagramWidth, key.theme == DiagramKey.hash(diagramTheme) else { return }
		let rows = IndexSet(entries.indices.filter { row in
			guard let item = entries[row].item, item.state != .streaming else { return false }
			return documents[item.sequence]?.document.diagrams.contains(key.source) ?? false
		})
		guard !rows.isEmpty else { return }
		pending.formUnion(rows)
		let visible = visibleRows.intersection(rows)
		if !visible.isEmpty {
			let anchor = following ? nil : seenPin(visibleRows, besides: visible)
			table.reloadData(forRowIndexes: visible, columnIndexes: [0])
			let changed = measure(visible)
			if !changed.isEmpty { noteHeights(changed, keeping: anchor) }
		}
		scheduleIdle()
	}

	private func findPending() {
		let width = width
		pending = IndexSet(entries.indices.filter { !heights.isExact(entries[$0], expanded: isExpanded($0), diagrams: diagramPrint($0), width: width) })
	}

	/// The item rows, as table rows.
	private var itemRows: [Int] { entries.indices.filter { entries[$0].item != nil } }

	// MARK: Expansion

	/// Expands or collapses a thinking or tool row, keeping its top where it is: its body opens or closes below it.
	func toggle(_ sequence: UInt64) {
		guard let row = entries.firstIndex(where: { $0.key == .item(sequence) }), model(row).collapsible else { return }
		if expanded.remove(sequence) == nil { expanded.insert(sequence) }
		// The row the reader clicked stays put, even at the bottom: its body opens below it, and following the bottom
		// resumes only if the bottom is still in view.
		let anchor = pin(row)
		following = false
		pending.insert(row)
		table.reloadData(forRowIndexes: [row], columnIndexes: [0])
		// The table has the other state's height, whatever the cache estimated for this one.
		_ = measure([row])
		noteHeights([row], keeping: anchor)
		following = distanceFromBottom < Self.followSlack
		reportScrollLater()
	}

	/// Whether the first row of `kind` is expanded (the test bridge's `thinkingOpen` and `toolOpen`).
	func isFirstExpanded(_ kind: TranscriptRowModel.Kind) -> Bool {
		itemRows.first { model($0).kind == kind }.map(isExpanded) ?? false
	}

	/// Toggles the first row of `kind`, as clicking its summary does (the test bridge's `toggleThinking` and `toggleTool`).
	func toggleFirst(_ kind: TranscriptRowModel.Kind) {
		if let row = itemRows.first(where: { model($0).kind == kind }), let item = entries[row].item { toggle(item.sequence) }
	}

	/// Measures `rows` that aren't exact yet, and returns those whose height changed, for the table to be told.
	private func measure(_ rows: some Sequence<Int>) -> IndexSet {
		let width = width
		var changed = IndexSet()
		for row in rows where pending.contains(row) {
			let entry = entries[row]
			let expanded = isExpanded(row)
			let before = heights.height(entry, expanded: expanded, width: width)
			measurer.rootView = AnyView(view(row).frame(width: width))
			measurements += 1
			let height = max(1, ceil(measurer.fittingSize.height))
			heights.record(entry, expanded: expanded, diagrams: diagramPrint(row), width: width, height: height)
			pending.remove(row)
			if height != before { changed.insert(row) }
		}
		return changed
	}

	/// Makes the visible rows exact, at once: the reader never sees an estimated row settle. The corrections keep `kept`
	/// in place, or else the rows the reader already saw.
	private func measureVisible(keeping kept: Pin? = nil) {
		guard !pending.isEmpty else { return }
		for _ in 0..<3 {
			let visible = visibleRows
			let fresh = visible.intersection(pending)
			let anchor = kept ?? seenPin(visible, besides: fresh)
			let changed = measure(fresh)
			guard !changed.isEmpty else { return }
			noteHeights(changed, keeping: anchor)
		}
	}

	private var visibleRows: IndexSet {
		let visible = table.rows(in: contentView.bounds)
		return IndexSet(integersIn: visible.location..<(visible.location + visible.length))
	}

	/// The first visible row that was exact before `fresh` were measured, so a correction grows toward the side the reader
	/// hasn't seen: scrolling up, the row that just came into view at the top settles upward, and the rows below it stay.
	/// With no such row (a jump to rows never seen), the top visible row.
	private func seenPin(_ visible: IndexSet, besides fresh: IndexSet) -> Pin? {
		visible.subtracting(fresh).first.map(pin) ?? visiblePin
	}

	/// Applies new heights without moving what the reader sees: the bottom while following, otherwise `anchor`.
	private func noteHeights(_ rows: IndexSet, keeping anchor: Pin?) {
		adjusting = true
		withoutAnimation { table.noteHeightOfRows(withIndexesChanged: rows) }
		settle(anchor)
		adjusting = false
	}

	/// `noteHeightOfRows` animates the rows to their new heights unless told not to; a correction must not show.
	private func withoutAnimation(_ change: () -> Void) {
		NSAnimationContext.runAnimationGroup { context in
			context.duration = 0
			context.allowsImplicitAnimation = false
			change()
		}
	}

	private func settle(_ anchor: Pin?) {
		// Only the geometry: the rows' rects and the table's height. The cells lay out when they draw.
		table.tile()
		if following {
			scrollToBottom()
		} else if let anchor, let row = entries.firstIndex(where: { $0.key == anchor.key }) {
			setTop(table.rect(ofRow: row).minY - anchor.offset)
		}
	}

	// MARK: Idle measurement

	private func scheduleIdle() {
		guard !pending.isEmpty, idle == nil, window != nil, !inLiveResize else { return }
		let link = displayLink(target: self, selector: #selector(measureIdle))
		link.add(to: .main, forMode: .common)
		idle = link
	}

	private func stopIdle() {
		idle?.invalidate()
		idle = nil
	}

	@objc func measureIdle() {
		guard !pending.isEmpty, !inLiveResize else { return stopIdle() }
		let start = CACurrentMediaTime()
		let visible = visibleRows
		let anchor = seenPin(visible, besides: visible.intersection(pending))
		let center = ((visible.first ?? 0) + (visible.last ?? 0)) / 2
		var changed = IndexSet()
		while CACurrentMediaTime() - start < Self.measureBudget, let row = nearestPending(to: center) {
			changed.formUnion(measure([row]))
		}
		if !changed.isEmpty { noteHeights(changed, keeping: anchor) }
		if pending.isEmpty { stopIdle() }
	}

	private func nearestPending(to row: Int) -> Int? {
		let below = pending.integerGreaterThanOrEqualTo(row)
		let above = pending.integerLessThan(row)
		switch (above, below) {
		case (let above?, let below?): return row - above <= below - row ? above : below
		case (let above?, nil): return above
		case (nil, let below): return below
		}
	}

	override func viewDidMoveToWindow() {
		super.viewDidMoveToWindow()
		if window == nil { stopIdle() } else { scheduleIdle() }
		watchWheel()
	}

	// MARK: Wheel

	/// Takes the vertical scroll gestures over the transcript before what is under the pointer (a code block) can
	/// swallow them (`WheelRouting`).
	private func watchWheel() {
		if let wheelMonitor { NSEvent.removeMonitor(wheelMonitor) }
		wheelMonitor = nil
		guard window != nil else { return }
		wheelMonitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
			let taken = MainActor.assumeIsolated { self?.takeWheel(event) ?? false }
			return taken ? nil : event
		}
	}

	private func takeWheel(_ event: NSEvent) -> Bool {
		guard let window, event.windowNumber == window.windowNumber, isUnderPointer(at: event.locationInWindow) else { return false }
		let toTranscript = wheelRouting.toTranscript(
			phase: event.phase, momentumPhase: event.momentumPhase,
			deltaX: event.scrollingDeltaX, deltaY: event.scrollingDeltaY)
		guard toTranscript else { return false }
		scrollWheel(with: event)
		return true
	}

	/// Whether the window, hit-testing `point` (in its coordinates), lands in this transcript. Not when another tab is
	/// active (every tab stays in the view tree, invisible and not hit-testable), nor under a layer drawn over it (a hub,
	/// the palette): being inside its frame isn't enough.
	func isUnderPointer(at point: NSPoint) -> Bool {
		guard let content = window?.contentView else { return false }
		// `hitTest` takes a point in the receiver's superview's coordinates.
		let hit = content.hitTest(content.superview?.convert(point, from: nil) ?? point)
		return hit?.isDescendant(of: self) ?? false
	}

	// MARK: Width

	@objc private func frameChanged() {
		guard width != laidOutWidth else { return }
		laidOutWidth = width
		guard !entries.isEmpty else { return }
		// Every height is stale at a new width: estimates (the old heights) for all, measurements for the visible
		// rows now, and the rest after a live resize ends.
		let anchor = following ? nil : visiblePin
		findPending()
		adjusting = true
		withoutAnimation { table.noteHeightOfRows(withIndexesChanged: IndexSet(entries.indices)) }
		settle(anchor)
		adjusting = false
		measureVisible(keeping: anchor)
		scheduleIdle()
	}

	override func viewDidEndLiveResize() {
		super.viewDidEndLiveResize()
		// The visible rows' diagrams, which waited for the resize to end, are drawn at the width it ended at.
		for row in visibleRows where entries[row].item != nil { _ = model(row) }
		scheduleIdle()
	}

	// MARK: Scrolling

	@objc private func boundsChanged() {
		let top = contentView.bounds.minY
		if top >= Self.olderSlack {
			// Away from the top again: a request that brought nothing (an error) may go again on the way back.
			olderRequested = false
		}
		// Only the reader's own scrolls count: not the view's adjustments, nor a resize or layout of the clip view.
		let byReader = !adjusting && top != lastTop
		if byReader {
			lastTop = top
			following = distanceFromBottom < Self.followSlack
		}
		// After `following` is the reader's: the rows coming into view settle around what they now look at.
		measureVisible()
		guard byReader else { return }
		reportScrollLater()
		if top < Self.olderSlack, hasOlder, !olderRequested, table.bounds.height > contentView.bounds.height {
			olderRequested = true
			// The anchor goes first: a chat reading older history keeps it, rather than trimming to its tail.
			onScroll(visibleAnchor)
			onNeedsOlder()
		}
	}

	private func reportScrollLater() {
		scrollReport?.cancel()
		scrollReport = Task { [weak self] in
			do { try await Task.sleep(for: .milliseconds(250)) } catch { return }
			guard let self else { return }
			self.onScroll(self.following ? nil : self.visibleAnchor)
		}
	}

	func scrollToBottom() {
		setTop(table.bounds.height - contentView.bounds.height)
	}

	/// Scrolls as the reader would, to the very top (the test bridge's `scrollTop`).
	func scrollToTop() {
		contentView.scroll(to: .zero)
		reflectScrolledClipView(contentView)
	}

	private func setTop(_ y: CGFloat) {
		let top = min(max(0, table.bounds.height - contentView.bounds.height), max(0, y))
		guard top != contentView.bounds.minY else { return }
		let wasAdjusting = adjusting
		adjusting = true
		contentView.scroll(to: NSPoint(x: 0, y: top))
		reflectScrolledClipView(contentView)
		lastTop = top
		adjusting = wasAdjusting
	}

	/// What the test bridge reads: every item row's sequence, kind (the web transcript's `chat-<kind>` class), text and
	/// top relative to the visible area's top, the cards, every row's text in order, how many diagrams are drawn, failed,
	/// pending or shown as their source while streaming, and the web transcript's other fields. Nothing native runs a row's content, so there are no scripts or unsafe attributes to count.
	func observation() -> [String: Any] {
		let top = contentView.bounds.minY
		var rows: [[String: Any]] = []
		var texts: [String] = []
		var cards = 0
		var diagrams = ["drawn": 0, "failed": 0, "pending": 0, "source": 0]
		for row in entries.indices {
			switch entries[row] {
			case .item(let item):
				let model = model(row)
				rows.append([
					"sequence": String(item.sequence), "kind": "chat-item chat-\(model.kind.rawValue)",
					"text": model.plainText, "offset": Int((table.rect(ofRow: row).minY - top).rounded()),
				])
				texts.append(model.plainText)
				for source in model.document?.diagrams ?? [] {
					switch model.diagrams?[source] {
					case .drawn: diagrams["drawn", default: 0] += 1
					case .failed: diagrams["failed", default: 0] += 1
					case .pending: diagrams["pending", default: 0] += 1
					case nil: diagrams["source", default: 0] += 1
					}
				}
			case .card(let card):
				cards += 1
				texts.append(card.plainText)
			}
		}
		let verdict = items.lazy.compactMap { item -> String? in
			if case .tool(_, _, _, let verdict?, _) = item.content { return "Judgess \(verdict.kind)" }
			return nil
		}.first
		return [
			"rows": rows, "cards": cards, "text": texts.joined(separator: "\n"), "diagrams": diagrams,
			"thinkingOpen": isFirstExpanded(.thinking), "toolOpen": isFirstExpanded(.tool),
			"verdict": verdict ?? "", "scripts": 0, "unsafe": 0,
			"top": Int(top.rounded()), "topSequence": visibleAnchor.map { String($0.sequence) as Any } ?? NSNull(),
			"settled": isSettled, "measurements": measurements,
		]
	}
}

/// What one table row draws: an item's row, or a permission card.
struct TranscriptEntryView: View {
	enum Model: Equatable {
		case row(TranscriptRowModel)
		case card(PermissionCard)
	}

	let model: Model
	var toggle: () -> Void = {}
	var answer: (String) -> Void = { _ in }

	var body: some View {
		switch model {
		case .row(let row): TranscriptRow(model: row, toggle: toggle)
		case .card(let card): PermissionCardView(card: card, answer: answer)
		}
	}
}

/// A recycled table cell hosting one SwiftUI row, or nothing while it waits to be reused.
final class TranscriptCell: NSTableCellView {
	private let host = NSHostingView<TranscriptEntryView?>(rootView: nil)

	init(identifier: NSUserInterfaceItemIdentifier) {
		super.init(frame: .zero)
		self.identifier = identifier
		host.sizingOptions = []
		// A row has no safe area to keep clear, and tracking one costs every row on every scroll.
		host.safeAreaRegions = []
		host.frame = bounds
		host.autoresizingMask = [.width, .height]
		addSubview(host)
	}

	required init?(coder: NSCoder) { fatalError("not used") }

	/// Whether the cell shows a row (rather than waiting for reuse).
	var isShowing: Bool { host.rootView != nil }

	func show(_ view: TranscriptEntryView) {
		guard view.model != host.rootView?.model else { return }
		host.rootView = view
	}

	/// Shows nothing, so that a cell waiting for reuse exposes no stale row to accessibility.
	func clear() {
		host.rootView = nil
	}
}

/// The native transcript in SwiftUI: one `TranscriptScrollView` per chat panel.
struct TranscriptView: NSViewRepresentable {
	let items: [TranscriptItem]
	var questions: [PermissionQuestion] = []
	var generation: UInt64 = 0
	var theme: [String: String] = [:]
	let hasOlder: Bool
	let anchor: ChatScrollAnchor?
	var onScroll: (ChatScrollAnchor?) -> Void
	var onNeedsOlder: () -> Void
	var onAnswer: (String, String, UInt64) -> Void = { _, _, _ in }
	var onView: (TranscriptScrollView) -> Void

	func makeNSView(context: Context) -> TranscriptScrollView {
		TranscriptScrollView(anchor: anchor)
	}

	func updateNSView(_ view: TranscriptScrollView, context: Context) {
		view.onScroll = onScroll
		view.onNeedsOlder = onNeedsOlder
		view.onAnswer = onAnswer
		view.diagramTheme = theme
		view.hasOlder = hasOlder
		view.apply(items, questions: questions, generation: generation)
		onView(view)
	}
}
