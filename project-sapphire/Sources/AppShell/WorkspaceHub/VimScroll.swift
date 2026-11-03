import Layers
import SwiftUI

/// A scroll motion shared by native and web pages; pending consumes the first `g` without moving.
enum VimMotion: Equatable {
	case line(Int), halfPage(Int), top, bottom, pending
}

/// Interprets ADR-004's keys and owns the pending `g` state for one page.
struct VimKeyParser {
	private var pendingG = false

	mutating func motion(for key: Layers.KeyPress) -> VimMotion? {
		let completingG = pendingG
		pendingG = false
		if key.modifiers.isEmpty, key.characters == "g" {
			if completingG { return .top }
			pendingG = true
			return .pending
		}
		if key.modifiers.isDisjoint(with: [.control, .option, .command]), key.characters == "G" { return .bottom }
		if key.modifiers.isEmpty {
			if key.characters == "j" || key.keyCode == KeyCode.downArrow { return .line(1) }
			if key.characters == "k" || key.keyCode == KeyCode.upArrow { return .line(-1) }
		}
		if key.modifiers == .control {
			if key.keyCode == KeyCode.d { return .halfPage(1) }
			if key.keyCode == KeyCode.u { return .halfPage(-1) }
		}
		return nil
	}
}

/// Vim-style scrolling of a native read-only scroll view (ADR-004).
@MainActor
@Observable
final class VimScroll {
	var position = ScrollPosition(edge: .top)
	var geometry = Geometry()
	@ObservationIgnored private var keys = VimKeyParser()

	struct Geometry: Equatable {
		var offset: CGFloat = 0
		var contentHeight: CGFloat = 0
		var viewportHeight: CGFloat = 0
		var maxOffset: CGFloat { max(contentHeight - viewportHeight, 0) }
	}

	/// Returns whether the key was a scroll motion, which consumes it.
	func handle(_ key: Layers.KeyPress) -> Bool {
		guard let motion = keys.motion(for: key) else { return false }
		switch motion {
		case .pending: break
		case .top: position.scrollTo(edge: .top)
		case .bottom: position.scrollTo(edge: .bottom)
		case .line(let direction): move(CGFloat(direction) * geometry.viewportHeight * 0.1)
		case .halfPage(let direction): move(CGFloat(direction) * geometry.viewportHeight * 0.5)
		}
		return true
	}

	private func move(_ delta: CGFloat) {
		let target = min(max(geometry.offset + delta, 0), geometry.maxOffset)
		withAnimation(.easeOut(duration: 0.15)) { position.scrollTo(y: target) }
	}

	/// The relative motion a key makes in a viewport this tall, or nil for other keys.
	nonisolated static func delta(for key: Layers.KeyPress, viewportHeight: CGFloat) -> CGFloat? {
		var keys = VimKeyParser()
		switch keys.motion(for: key) {
		case .line(let direction): return CGFloat(direction) * viewportHeight * 0.1
		case .halfPage(let direction): return CGFloat(direction) * viewportHeight * 0.5
		default: return nil
		}
	}
}
