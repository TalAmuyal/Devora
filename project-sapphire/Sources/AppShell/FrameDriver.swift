import AppKit
import QuartzCore
import Store
import SwiftUI

/// Folds the store's pending core events once per display frame (ADR-010), from the window's display link.
///
/// The link only runs while events are pending, so an idle window doesn't wake the main thread every frame.
struct FrameDriver: NSViewRepresentable {
	let store: Store

	func makeNSView(context: Context) -> FrameDriverView {
		FrameDriverView(store: store)
	}

	func updateNSView(_ view: FrameDriverView, context: Context) {}
}

final class FrameDriverView: NSView {
	private let store: Store
	private var link: CADisplayLink?

	init(store: Store) {
		self.store = store
		super.init(frame: .zero)
	}

	@available(*, unavailable)
	required init?(coder: NSCoder) { fatalError() }

	override func viewDidMoveToWindow() {
		super.viewDidMoveToWindow()
		// The link retains its target, so it must go when the view leaves its window.
		link?.invalidate()
		link = nil
		store.requestFrame = nil
		// A pending frame request dies with the link, and the inbox only requests one when it was empty, so fold now.
		store.foldPending()
		guard window != nil else { return }
		let link = displayLink(target: self, selector: #selector(tick(_:)))
		link.isPaused = true
		link.add(to: .main, forMode: .common)
		self.link = link
		store.requestFrame = { [weak link] in link?.isPaused = false }
	}

	@objc private func tick(_ link: CADisplayLink) {
		link.isPaused = true
		store.foldPending()
	}
}
