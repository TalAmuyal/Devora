import Layers
import SwiftUI

/// Vim-style scrolling of a read-only scroll view (ADR-004), as Ember's `createVimScroll`: `j`/`k` and the arrows move a tenth of the viewport, animated; `Ctrl+D`/`Ctrl+U` half of it; `gg` and `G` jump to either end.
///
/// The scroll view binds `position` and reports its geometry to `geometry`, which the motions are measured from.
@MainActor
@Observable
final class VimScroll {
	var position = ScrollPosition(edge: .top)
	var geometry = Geometry()
	/// After a lone `g`, until the next key shows whether it is `gg`.
	@ObservationIgnored private var pendingG = false

	struct Geometry: Equatable {
		var offset: CGFloat = 0
		var contentHeight: CGFloat = 0
		var viewportHeight: CGFloat = 0

		var maxOffset: CGFloat { max(contentHeight - viewportHeight, 0) }
	}

	/// Returns whether the key was a scroll motion, which consumes it.
	func handle(_ key: Layers.KeyPress) -> Bool {
		let completingG = pendingG
		pendingG = false
		let plain = key.modifiers.isEmpty
		if plain, key.characters == "g" {
			if completingG { position.scrollTo(edge: .top) } else { pendingG = true }
			return true
		}
		if key.modifiers.isDisjoint(with: [.control, .option, .command]), key.characters == "G" {
			position.scrollTo(edge: .bottom)
			return true
		}
		guard let delta = Self.delta(for: key, viewportHeight: geometry.viewportHeight) else { return false }
		let target = min(max(geometry.offset + delta, 0), geometry.maxOffset)
		withAnimation(.easeOut(duration: 0.15)) { position.scrollTo(y: target) }
		return true
	}

	/// The relative motion a key makes in a viewport this tall (positive is down), or nil when it isn't one.
	nonisolated static func delta(for key: Layers.KeyPress, viewportHeight: CGFloat) -> CGFloat? {
		if key.modifiers.isEmpty {
			if key.characters == "j" || key.keyCode == KeyCode.downArrow { return viewportHeight * 0.1 }
			if key.characters == "k" || key.keyCode == KeyCode.upArrow { return -viewportHeight * 0.1 }
		}
		if key.modifiers == .control {
			if key.keyCode == KeyCode.d { return viewportHeight / 2 }
			if key.keyCode == KeyCode.u { return -viewportHeight / 2 }
		}
		return nil
	}
}
