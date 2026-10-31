import Layers
import SwiftUI
import Testing

/// The routing and focus rules of ADR-003, on stacks of stand-in layers.
@MainActor
struct LayerRoutingTests {
	/// Records which layer took the focus, and which keys each layer saw.
	final class Log {
		var focused: [String] = []
		var keys: [String] = []
	}

	/// A value a test changes after handing it to a router.
	final class Box<Value> {
		var value: Value
		init(_ value: Value) { self.value = value }
	}

	let log = Log()
	let typed = KeyPress(keyCode: 0x00, characters: "a")
	let escape = KeyPress(keyCode: KeyCode.escape, characters: "\u{1B}")
	let q = KeyPress(keyCode: 0x0C, characters: "q")
	let ctrlRight = KeyPress(keyCode: KeyCode.rightArrow, modifiers: .control)

	/// A layer that takes the focus as `name`, logs every key it sees and claims those in `claims`.
	func layer(_ name: String, claims: Set<String> = [], preset: (LayerSpec) -> LayerSpec = { $0 }) -> LayerSpec {
		var spec = preset(LayerSpec(name: name) { EmptyView() })
		spec.focus = { [log] in log.focused.append(name) }
		spec.onKey = { [log] key in
			log.keys.append("\(name):\(key.characters)")
			return claims.contains(key.characters)
		}
		return spec
	}

	/// A router with one tab whose stack has a terminal at the bottom.
	func routerWithTab() -> (LayerRouter, tab: LayerStack) {
		let tab = Box<LayerStack?>(nil)
		let router = LayerRouter(activeTabStack: { tab.value }, fallbackFocus: { [log] in log.focused.append("nothing") })
		let stack = router.makeTabStack { true }
		tab.value = stack
		stack.push(layer("terminal", preset: terminalLayer))
		return (router, stack)
	}

	@Test func shortcutsRunBeforeAnyLayer() {
		let (router, _) = routerWithTab()
		router.windowStack.push(layer("hub", claims: [""], preset: pageLayer))
		router.shortcuts = { $0 == self.ctrlRight }

		#expect(router.handle(ctrlRight))
		#expect(log.keys.isEmpty, "a shortcut is consumed before any surface sees it")
	}

	@Test func aKeyNoLayerClaimsReachesTheTerminalUnconsumed() {
		let (router, tab) = routerWithTab()

		#expect(tab.route(typed) == .blocked)
		log.keys = []
		#expect(!router.handle(typed), "a blocked key is left for the first responder, the terminal")
		#expect(log.keys == ["terminal:a"])
	}

	@Test func anOpaqueWindowLayerKeepsKeysFromTheTab() {
		let (router, _) = routerWithTab()
		router.windowStack.push(layer("hub", claims: ["j"], preset: pageLayer))

		#expect(router.handle(KeyPress(keyCode: 0x26, characters: "j")))
		#expect(!router.handle(typed))
		#expect(log.keys == ["hub:j", "hub:a"], "the tab's stack sees neither key")
	}

	@Test func aTransparentLayerPassesWhatItDoesNotClaim() {
		let (router, _) = routerWithTab()
		router.windowStack.push(layer("dropdown", claims: ["j"], preset: popupLayer))

		#expect(router.windowStack.route(typed) == .passed)
		#expect(!router.handle(typed))
		#expect(log.keys == ["dropdown:a", "dropdown:a", "terminal:a"])
	}

	@Test func theTopLayerSeesKeysFirstAndAnOpaqueOneEndsTheWalk() {
		let (router, tab) = routerWithTab()
		tab.push(layer("progress", claims: ["c"], preset: pageLayer))

		#expect(router.handle(KeyPress(keyCode: 0x08, characters: "c")))
		#expect(!router.handle(typed))
		#expect(log.keys == ["progress:c", "progress:a"], "the terminal beneath an opaque layer sees nothing")
	}

	@Test func qAndEscapeDismissTheTopSurfaceButNeverTheTerminal() {
		let (router, tab) = routerWithTab()
		router.windowStack.push(layer("hub", preset: pageLayer))

		#expect(router.handle(escape))
		#expect(router.windowStack.isEmpty)
		#expect(!router.handle(q), "q is typing in a terminal")
		#expect(tab.layers.map(\.spec.name) == ["terminal"])
	}

	@Test func aChordIsNeverADismissal() {
		let (router, _) = routerWithTab()
		router.windowStack.push(layer("hub", preset: pageLayer))

		#expect(!router.handle(KeyPress(keyCode: KeyCode.escape, modifiers: .control, characters: "\u{1B}")))
		#expect(!router.windowStack.isEmpty)
	}

	@Test func aFocusedTextFieldKeepsQAndEscape() {
		let editing = Box(true)
		let router = LayerRouter(activeTabStack: { nil }, fallbackFocus: {}, isEditingText: { editing.value })
		router.windowStack.push(layer("palette", preset: pageLayer))

		#expect(!router.handle(q), "q types into the search field")
		#expect(!router.handle(escape), "Escape reaches the field's own handling")
		editing.value = false
		#expect(router.handle(escape))
		#expect(router.windowStack.isEmpty)
	}

	@Test func aModalKeepsTabWithinItself() {
		let (router, _) = routerWithTab()
		router.windowStack.push(layer("hub", preset: pageLayer))
		var cycles: [Bool] = []
		var dialog = layer("dialog", preset: modalLayer)
		dialog.cycleFocus = { cycles.append($0) }
		router.windowStack.push(dialog)
		log.keys = []
		let tab = KeyPress(keyCode: KeyCode.tab, characters: "\t")

		#expect(router.handle(tab))
		#expect(router.handle(KeyPress(keyCode: KeyCode.tab, modifiers: .shift, characters: "\u{19}")))
		#expect(cycles == [false, true])
		#expect(log.keys.isEmpty, "neither the dialog's keys nor the hub beneath see Tab")

		router.windowStack.pop()
		#expect(!router.handle(tab), "without the dialog, Tab goes on to the window")
	}

	@Test func aVetoedDismissalIsConsumedAndKeepsTheLayer() {
		let (router, _) = routerWithTab()
		var spec = layer("welcome", preset: pageLayer)
		spec.onUserDismissRequest = { .veto }
		router.windowStack.push(spec)

		#expect(router.handle(escape))
		#expect(router.windowStack.layers.map(\.spec.name) == ["welcome"])
	}

	@Test func theTopLayerTakesTheFocusAndAnEmptiedWindowStackReturnsItToTheActiveTab() {
		let (router, tab) = routerWithTab()
		router.windowStack.push(layer("hub", preset: pageLayer))
		tab.push(layer("progress", preset: pageLayer))
		router.windowStack.push(layer("dialog", preset: modalLayer))

		router.windowStack.pop()
		router.windowStack.pop()

		#expect(log.focused == ["terminal", "hub", "dialog", "hub", "progress"], "the tab took no focus while the hub was open")
	}

	@Test func aTabStackTakesTheFocusOnlyWhileItsTabIsActive() {
		let active = Box(false)
		let router = LayerRouter(activeTabStack: { nil }, fallbackFocus: {})
		let tab = router.makeTabStack { active.value }

		tab.push(layer("background terminal", preset: terminalLayer))
		active.value = true
		tab.focusTop()

		#expect(log.focused == ["background terminal"])
	}

	@Test func closingAPopupLeavesTheFocusWhereItWas() {
		let (router, _) = routerWithTab()
		router.windowStack.push(layer("hub", preset: pageLayer))
		router.windowStack.push(layer("dropdown", preset: popupLayer))

		router.windowStack.pop()

		#expect(log.focused == ["terminal", "hub"])
	}

	@Test func withNothingToFocusTheFocusIsTakenAway() {
		let router = LayerRouter(activeTabStack: { nil }, fallbackFocus: { [log] in log.focused.append("nothing") })
		router.windowStack.push(pageLayer(LayerSpec(name: "page") { EmptyView() }))
		router.windowStack.pop()

		#expect(log.focused == ["nothing", "nothing"], "a layer without a focus target, then an empty window and no tab")
	}

	@Test func onlyNavigationRefreshesTheRevealedLayer() {
		let stack = LayerStack()
		var refreshes = 0
		var hub = layer("hub", preset: pageLayer)
		hub.onReveal = { refreshes += 1 }
		stack.push(hub)

		stack.push(layer("dialog", preset: modalLayer))
		stack.pop()
		stack.push(layer("settings", preset: pageLayer))
		stack.pop()

		#expect(refreshes == 1)
	}

	@Test func everyRemovalCleansUpExactlyOnce() {
		let stack = LayerStack()
		var cleaned: [String] = []
		func tracked(_ name: String) -> LayerSpec {
			var spec = layer(name)
			spec.onCleanup = { cleaned.append(name) }
			return spec
		}
		let a = stack.push(tracked("a"))
		stack.push(tracked("b"))
		stack.replaceTop(tracked("c"))
		stack.remove(a)
		stack.remove(a)
		stack.push(tracked("d"))
		stack.clear()

		#expect(cleaned == ["b", "a", "d", "c"])
	}
}
