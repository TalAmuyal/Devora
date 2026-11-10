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
	private(set) var items: [TranscriptItem] = []
	var hasOlder = false
	/// Where the reader is, 250 ms after a scroll: the top row and its offset, or nil at the bottom.
	var onScroll: (ChatScrollAnchor?) -> Void = { _ in }
	var onNeedsOlder: () -> Void = {}

	private var heights = RowHeights()
	private var documents: [UInt64: (text: String, document: MarkdownDocument)] = [:]
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
	private(set) var measurements = 0

	init(anchor: ChatScrollAnchor?) {
		restoring = anchor
		following = anchor == nil
		super.init(frame: .zero)
		measurer.safeAreaRegions = []
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
		idle?.invalidate()
		scrollReport?.cancel()
		if let wheelMonitor { NSEvent.removeMonitor(wheelMonitor) }
	}

	var width: CGFloat { max(200, contentView.bounds.width) }

	var distanceFromBottom: CGFloat { max(0, table.bounds.height - contentView.bounds.maxY) }

	/// The first row whose bottom is below the visible area's top, and how far its top is from that top.
	var visibleAnchor: ChatScrollAnchor? {
		visiblePin.map { ChatScrollAnchor(sequence: $0.sequence, offset: Int32($0.offset.rounded())) }
	}

	/// A row kept in place across an update, with its exact offset: a rounded one would drift with every correction.
	private struct Pin {
		let sequence: UInt64
		let offset: CGFloat
	}

	private func pin(_ row: Int) -> Pin {
		Pin(sequence: items[row].sequence, offset: table.rect(ofRow: row).minY - contentView.bounds.minY)
	}

	private var visiblePin: Pin? {
		let row = table.row(at: NSPoint(x: 1, y: contentView.bounds.minY + 0.5))
		return row >= 0 && row < items.count ? pin(row) : nil
	}

	/// Whether every row has its measured height (the idle measurement has nothing left to do).
	var isSettled: Bool { pending.isEmpty && idle == nil }

	// MARK: Updates

	func apply(_ new: [TranscriptItem]) {
		let change = TranscriptChange(from: items, to: new)
		guard change != .none else { return }
		let anchor = following ? nil : visiblePin
		let firstLoad = items.isEmpty
		items = new
		let sequences = Set(new.map(\.sequence))
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
		heights.keep(only: sequences)
		findPending()
		var kept: Pin?
		if firstLoad, let saved = restoring {
			// The saved position, once: at its row if the transcript has it, otherwise at the bottom, as the web one does.
			// Its row stays put while the rows around it are measured.
			restoring = nil
			following = !items.contains { $0.sequence == saved.sequence }
			kept = following ? nil : Pin(sequence: saved.sequence, offset: CGFloat(saved.offset))
			settle(kept)
		}
		measureVisible(keeping: kept)
		settle(anchor)
		scheduleIdle()
	}

	// MARK: Heights

	func numberOfRows(in tableView: NSTableView) -> Int { items.count }

	func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
		heights.height(items[row], width: width)
	}

	func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
		let identifier = NSUserInterfaceItemIdentifier("transcript-row")
		let cell = tableView.makeView(withIdentifier: identifier, owner: nil) as? TranscriptCell ?? TranscriptCell(identifier: identifier)
		cell.show(model(row))
		return cell
	}

	func tableView(_ tableView: NSTableView, shouldSelectRow row: Int) -> Bool { false }

	private func model(_ row: Int) -> TranscriptRowModel {
		let item = items[row]
		guard case .assistant(let text) = item.content else { return TranscriptRowModel(item, document: nil) }
		if let cached = documents[item.sequence], cached.text == text { return TranscriptRowModel(item, document: cached.document) }
		let document = MarkdownDocument(text)
		documents[item.sequence] = (text, document)
		return TranscriptRowModel(item, document: document)
	}

	private func findPending() {
		let width = width
		pending = IndexSet(items.indices.filter { !heights.isExact(items[$0], width: width) })
	}

	/// Measures `rows` that aren't exact yet, and returns those whose height changed, for the table to be told.
	private func measure(_ rows: some Sequence<Int>) -> IndexSet {
		let width = width
		var changed = IndexSet()
		for row in rows where pending.contains(row) {
			let item = items[row]
			let before = heights.height(item, width: width)
			measurer.rootView = AnyView(TranscriptRow(model: model(row)).frame(width: width))
			measurements += 1
			let height = max(1, ceil(measurer.fittingSize.height))
			heights.record(item, width: width, height: height)
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
		} else if let anchor, let row = items.firstIndex(where: { $0.sequence == anchor.sequence }) {
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
		guard let window, event.windowNumber == window.windowNumber,
			  visibleRect.contains(convert(event.locationInWindow, from: nil)) else { return false }
		let toTranscript = wheelRouting.toTranscript(
			phase: event.phase, momentumPhase: event.momentumPhase,
			deltaX: event.scrollingDeltaX, deltaY: event.scrollingDeltaY)
		guard toTranscript else { return false }
		scrollWheel(with: event)
		return true
	}

	// MARK: Width

	@objc private func frameChanged() {
		guard width != laidOutWidth else { return }
		laidOutWidth = width
		guard !items.isEmpty else { return }
		// Every height is stale at a new width: estimates (the old heights) for all, measurements for the visible
		// rows now, and the rest after a live resize ends.
		let anchor = following ? nil : visiblePin
		findPending()
		adjusting = true
		withoutAnimation { table.noteHeightOfRows(withIndexesChanged: IndexSet(items.indices)) }
		settle(anchor)
		adjusting = false
		measureVisible(keeping: anchor)
		scheduleIdle()
	}

	override func viewDidEndLiveResize() {
		super.viewDidEndLiveResize()
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

	/// What the test bridge reads: every row's sequence, text and top relative to the visible area's top.
	func observation() -> [String: Any] {
		let top = contentView.bounds.minY
		let rows = items.indices.map { row -> [String: Any] in
			["sequence": String(items[row].sequence), "text": model(row).plainText, "offset": Int((table.rect(ofRow: row).minY - top).rounded())]
		}
		return [
			"rows": rows, "cards": 0, "thinkingOpen": false, "toolOpen": false, "verdict": "",
			"top": Int(top.rounded()), "topSequence": visibleAnchor.map { String($0.sequence) as Any } ?? NSNull(),
			"settled": isSettled, "measurements": measurements,
		]
	}
}

/// A recycled table cell hosting one SwiftUI row.
final class TranscriptCell: NSTableCellView {
	private let host = NSHostingView(rootView: TranscriptRow(model: TranscriptRowModel(
		TranscriptItem(sequence: 0, cliSession: "", parentToolUseId: nil, state: .complete, content: .assistant(text: "")),
		document: nil)))
	private var shown: TranscriptRowModel?

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

	func show(_ model: TranscriptRowModel) {
		guard model != shown else { return }
		shown = model
		host.rootView = TranscriptRow(model: model)
	}
}

/// The native transcript in SwiftUI: one `TranscriptScrollView` per chat panel.
struct TranscriptView: NSViewRepresentable {
	let items: [TranscriptItem]
	let hasOlder: Bool
	let anchor: ChatScrollAnchor?
	var onScroll: (ChatScrollAnchor?) -> Void
	var onNeedsOlder: () -> Void
	var onView: (TranscriptScrollView) -> Void

	func makeNSView(context: Context) -> TranscriptScrollView {
		TranscriptScrollView(anchor: anchor)
	}

	func updateNSView(_ view: TranscriptScrollView, context: Context) {
		view.onScroll = onScroll
		view.onNeedsOlder = onNeedsOlder
		view.hasOlder = hasOlder
		view.apply(items)
		onView(view)
	}
}
