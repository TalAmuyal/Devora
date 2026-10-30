import Observation

/// Identifies a pushed layer, for removing it later.
public struct LayerHandle: Hashable, Sendable {
	let id: Int
}

/// An ordered stack of surfaces that owns their order, focus, dismissal and key walk; `LayerHost` draws it (see `CLAUDE.md`).
///
/// It is a mechanism with no domain knowledge and no key monitor of its own: `LayerRouter` owns the one entry point and calls `route`.
/// There is one stack per tab plus one for the window, which is what decides how far a surface extends.
@MainActor
@Observable
public final class LayerStack {
	public struct Layer: Identifiable {
		public let handle: LayerHandle
		public let spec: LayerSpec
		public var id: LayerHandle { handle }
	}

	/// Bottom to top.
	public private(set) var layers: [Layer] = []

	@ObservationIgnored private var nextID = 0
	@ObservationIgnored private let isFocusOwner: @MainActor () -> Bool
	@ObservationIgnored private let fallbackFocus: @MainActor () -> Void
	@ObservationIgnored private let isEditingText: @MainActor () -> Bool
	/// Runs when the last layer goes; the window stack uses it to hand the focus back to the active tab.
	@ObservationIgnored public var onEmptied: (@MainActor () -> Void)?

	/// - Parameters:
	///   - isFocusOwner: whether this stack's top may take the keyboard focus now (a tab's stack only while its tab is active and no window surface is open).
	///   - fallbackFocus: takes the focus away from whatever has it, for a top layer that has no focus target of its own.
	///   - isEditingText: whether a text field or text view has the focus, which keeps `q` and Escape from dismissing.
	public init(
		isFocusOwner: @escaping @MainActor () -> Bool = { true },
		fallbackFocus: @escaping @MainActor () -> Void = {},
		isEditingText: @escaping @MainActor () -> Bool = { false }
	) {
		self.isFocusOwner = isFocusOwner
		self.fallbackFocus = fallbackFocus
		self.isEditingText = isEditingText
	}

	public var isEmpty: Bool { layers.isEmpty }
	public var top: Layer? { layers.last }

	/// Whether any layer here owns the keyboard: the precondition of the app-wide shortcuts that open a window surface or switch tabs.
	public var hasOpaqueLayer: Bool { layers.contains { $0.spec.opaque } }

	public func find(_ name: String) -> LayerHandle? {
		layers.first { $0.spec.name == name }?.handle
	}

	/// Pushes a layer on top, giving it the focus unless it declines.
	@discardableResult
	public func push(_ spec: LayerSpec) -> LayerHandle {
		nextID += 1
		let layer = Layer(handle: LayerHandle(id: nextID), spec: spec)
		layers.append(layer)
		if spec.takesFocus { focus(layer) }
		return layer.handle
	}

	public func pop() {
		if let top { remove(top.handle) }
	}

	/// Removes a layer at any position; only removing the top reveals, and refocuses, what is beneath.
	public func remove(_ handle: LayerHandle) {
		let wasTop = top?.handle == handle
		guard let removed = detach(handle) else { return }
		if wasTop { reveal(after: removed.spec) }
	}

	/// Swaps the top layer: the old one's cleanup runs, the new one takes the focus, and nothing below is revealed.
	@discardableResult
	public func replaceTop(_ spec: LayerSpec) -> LayerHandle {
		if let top { detach(top.handle) }
		return push(spec)
	}

	/// Runs the top layer's dismissal, as `q` or Escape would; returns whether there was a layer to ask.
	@discardableResult
	public func requestUserDismiss() -> Bool {
		guard let top else { return false }
		dismiss(top)
		return true
	}

	/// Teardown: every layer's cleanup, top first, with no reveal and no focus change.
	public func clear() {
		while let top { detach(top.handle) }
	}

	/// Focuses the top layer; returns false when there is none, so the caller decides where the focus goes instead.
	@discardableResult
	public func focusTop() -> Bool {
		guard let top else { return false }
		focus(top)
		return true
	}

	/// Walks the stack top-down and reports what happened; the router acts on the event.
	public func route(_ key: KeyPress) -> RouteResult {
		for layer in layers.reversed() {
			if layer.spec.onKey?(key) == true { return .consumed }
			if layer.spec.dismissible, Self.isDismissKey(key) {
				// A focused text field keeps the key: `q` types, and Escape reaches the field's own handling.
				if isEditingText() { return .blocked }
				dismiss(layer)
				return .consumed
			}
			if layer.spec.opaque { return .blocked }
		}
		return .passed
	}

	/// `q` and Escape, the vim-style pair; with a modifier they are shortcuts, never dismissals.
	static func isDismissKey(_ key: KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		return key.keyCode == KeyCode.escape || key.characters == "q"
	}

	private func dismiss(_ layer: Layer) {
		if (layer.spec.onUserDismissRequest?() ?? .close) == .close {
			remove(layer.handle)
		}
	}

	private func focus(_ layer: Layer) {
		guard isFocusOwner() else { return }
		if let focus = layer.spec.focus { focus() } else { fallbackFocus() }
	}

	/// Splices the layer out and runs its cleanup once; nil if it is already gone.
	@discardableResult
	private func detach(_ handle: LayerHandle) -> Layer? {
		guard let index = layers.firstIndex(where: { $0.handle == handle }) else { return nil }
		let layer = layers.remove(at: index)
		layer.spec.onCleanup?()
		return layer
	}

	/// After the top layer goes: a layer that never took the focus never covered anything, so closing it changes nothing; otherwise the focus moves to what it revealed.
	private func reveal(after removed: LayerSpec) {
		guard removed.takesFocus else { return }
		guard let revealed = top else {
			onEmptied?()
			return
		}
		if removed.refreshesRevealedLayer { revealed.spec.onReveal?() }
		focus(revealed)
	}
}
