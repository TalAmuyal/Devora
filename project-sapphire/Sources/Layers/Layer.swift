import SwiftUI

/// What a stack did with a key:
/// - `consumed`: a layer acted on it, so the event goes no further.
/// - `blocked`: an opaque layer ended the walk without acting; routing stops, but the event is left alone, so it reaches the first responder (this is how a terminal layer receives typing).
/// - `passed`: no layer claimed it; the router may route it onward.
public enum RouteResult: Sendable, Equatable {
	case consumed
	case blocked
	case passed
}

/// A layer's answer to a dismissal request (`q` or Escape):
/// - `close`: pop this layer.
/// - `handled`: the layer did something internal instead; the key is consumed without popping.
/// - `veto`: refuse the dismissal; the key is still consumed.
public enum DismissDecision: Sendable, Equatable {
	case close
	case handled
	case veto
}

/// A stacked surface, described by its behavior; how far it extends comes from the stack it is pushed onto (see `CLAUDE.md`).
///
/// Call sites use the presets below (`pageLayer`, `modalLayer`, `popupLayer`, `terminalLayer`) rather than setting the flags by hand.
@MainActor
public struct LayerSpec {
	/// A stable name, e.g. `terminal` or `workspace-hub`.
	public var name: String
	/// What the stack draws for this layer, above the layers below it.
	public var content: AnyView
	/// Ends the key walk: keys this layer doesn't handle go no further down, nor to the stack below.
	/// A transparent layer (an anchored popup, a non-modal find bar) passes them on.
	public var opaque = true
	/// Takes the keyboard focus when it becomes the top layer.
	public var takesFocus = true
	/// `q` and Escape may dismiss it.
	public var dismissible = true
	/// Removing this layer from the top runs the revealed layer's `onReveal`: true for navigation (returning to a hub refreshes it), false for a transient sub-interaction such as a dialog.
	public var refreshesRevealedLayer = false
	/// Layer-specific keys, tried first in the walk; returning true consumes the key.
	public var onKey: ((KeyPress) -> Bool)?
	/// The answer to a user's dismissal request; `close` when absent.
	public var onUserDismissRequest: (() -> DismissDecision)?
	/// Runs exactly once, when the layer is popped, removed or replaced.
	public var onCleanup: (() -> Void)?
	/// Runs when this layer is the top again after a `refreshesRevealedLayer` layer above it is removed.
	public var onReveal: (() -> Void)?
	/// Gives the layer the keyboard focus; without it, the stack's fallback takes the focus away from whatever had it, so keys can't reach a covered surface.
	public var focus: (() -> Void)?

	public init<Content: View>(name: String, @ViewBuilder content: () -> Content) {
		self.name = name
		self.content = AnyView(content())
	}
}

/// A full surface that replaces its stack's region (a hub, the User Guide, the Command Palette); dismissing it refreshes what it covered, since opening it was navigation.
@MainActor
public func pageLayer(_ spec: LayerSpec) -> LayerSpec {
	var spec = spec
	spec.refreshesRevealedLayer = true
	return spec
}

/// A dialog with the surface below kept visible as context; cancelling it doesn't refresh the caller, which refreshes on confirm.
/// Ember's modal also keeps Tab within the dialog; that comes with the first dialog.
@MainActor
public func modalLayer(_ spec: LayerSpec) -> LayerSpec {
	spec
}

/// An anchored light-dismiss surface (a dropdown): it never takes the focus and passes on the keys it doesn't handle, so the surface beneath keeps working while it is open.
@MainActor
public func popupLayer(_ spec: LayerSpec) -> LayerSpec {
	var spec = spec
	spec.opaque = false
	spec.takesFocus = false
	return spec
}

/// A terminal: the bottom layer of every tab's stack, and the shape a CLI app opened over a shell would take.
///
/// It is opaque but consumes nothing, so the walk ends here and the key reaches the terminal, which is the first responder.
/// It isn't dismissible: `q` belongs to the shell or to what it runs, and the process exiting is what removes the layer.
@MainActor
public func terminalLayer(_ spec: LayerSpec) -> LayerSpec {
	var spec = spec
	spec.dismissible = false
	return spec
}
