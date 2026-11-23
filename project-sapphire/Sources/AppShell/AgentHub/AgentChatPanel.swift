import AppKit
import CoreBindings
import Layers
import Observation
import Store
import SwiftUI
import TranscriptKit

@MainActor
@Observable
final class AgentPanelModel {
	let chat: AgentChat
	var prepared = false
	@ObservationIgnored weak var editor: NSTextView?
	@ObservationIgnored weak var transcript: TranscriptScrollView?
	@ObservationIgnored var onEditorReady: (() -> Void)?

	init(chat: AgentChat) {
		self.chat = chat
	}

	var pausePresentation: AgentPausePresentation {
		AgentPausePresentation(
			phase: chat.chat?.phase, pauseRequested: chat.chat?.pauseRequested == true,
			globallyPaused: chat.globallyPaused, globallyHeld: chat.chat?.globallyHeld == true,
			keepPausedSelected: chat.chat?.keepPaused == true)
	}

	var modelPresentation: AgentModelPresentation {
		AgentModelPresentation(model: chat.chat?.model, paused: chat.chat?.phase == .paused)
	}

	var contextPresentation: AgentContextPresentation {
		AgentContextPresentation(
			context: chat.chat?.context, phase: chat.chat?.phase, pauseRequested: chat.chat?.pauseRequested == true,
			globallyPaused: chat.globallyPaused, turnRunning: chat.chat?.turnRunning == true)
	}

	func focus() {
		Task { await chat.refreshSettings() }
		if let editor { editor.window?.makeFirstResponder(editor) }
	}

	func handle(_ key: Layers.KeyPress) -> Bool {
		// AppKit handles Control+Return as a context menu before NSTextView.keyDown.
		if let editor = editor as? PromptTextView, editor.window?.firstResponder === editor, editor.send(key) {
			return true
		}
		guard key.modifiers == .command || key.modifiers == [.command, .shift] else { return false }
		guard let current = chat.chat, let question = current.questions.first else { return false }
		let action: String
		switch key.keyCode {
		case 0x10: action = key.modifiers.contains(.shift) ? "always" : "allow"  // Y
		case 0x2D where key.modifiers == .command: action = "deny"  // N
		default: return false
		}
		Task { await chat.answer(question.requestId, action: action, generation: current.generation) }
		return true
	}
}

struct AgentChatPanel: View {
	static let layerName = "agent-chat"
	@Bindable var model: AgentPanelModel

	var body: some View {
		VStack(spacing: 0) {
			if let error = model.chat.chat?.error ?? model.chat.error {
				HStack {
					Text(error).textSelection(.enabled)
					Spacer()
					Button("Retry") { Task { await model.chat.retry() } }.disabled(model.chat.globallyPaused)
				}
				.padding(12).foregroundStyle(.orange)
			}
			if model.prepared {
				TranscriptView(
					items: model.chat.items.map { TranscriptKit.TranscriptItem($0, name: { model.chat.modelSuggestions?.name(of: $0) }) },
					questions: (model.chat.chat?.questions ?? []).map(TranscriptKit.PermissionQuestion.init),
					generation: model.chat.chat?.generation ?? 0, hasOlder: model.chat.hasOlder,
					anchor: model.chat.anchor.map(TranscriptKit.ChatScrollAnchor.init),
					showsSystem: model.chat.showsSystem,
					onScroll: { model.chat.saveScroll($0.map(CoreBindings.ChatScrollAnchor.init)) },
					onNeedsOlder: { Task { await model.chat.older() } },
					onAnswer: { id, action, generation in Task { await model.chat.answer(id, action: action, generation: generation) } },
					onView: { model.transcript = $0 })
				if let step = model.chat.chat?.step { AgentTurnStatus(step: step) }
			} else {
				ProgressView("Opening agent…").frame(maxWidth: .infinity, maxHeight: .infinity)
			}
			Divider()
			AgentPrompt(
				text: Binding(get: { model.chat.draft }, set: { model.chat.draft = $0 }),
				enterSends: model.chat.enterSends, onSend: { Task { await model.chat.send() } },
				onView: {
					model.editor = $0
					Task { model.onEditorReady?() }
				}
			)
			.frame(minHeight: 85, idealHeight: 110, maxHeight: 180).padding(8)
			AgentWidgetBar(
				context: model.chat.chat?.context,
				model: model.modelPresentation,
				modelNames: model.chat.modelSuggestions?.rows(for: .agentModel).map(\.name) ?? [], enterSends: model.chat.enterSends,
				showsSystem: model.chat.showsSystem, onShowsSystem: { model.chat.setShowsSystem($0) },
				pause: model.pausePresentation,
				compaction: model.contextPresentation,
				turnRunning: model.chat.chat?.turnRunning == true,
				canSend: model.chat.canSend, onSend: { Task { await model.chat.send() } },
				onStop: { Task { await model.chat.stop() } },
				onPause: { model.chat.pause() }, onResume: { Task { await model.chat.retry() } },
				onKeepPaused: { keep in Task { await model.chat.setKeepPaused(keep) } },
				onThreshold: { percent in Task { await model.chat.setCompactThreshold(percent) } },
				onCompactNow: { Task { await model.chat.compactNow() } }
			)
			.padding(.horizontal, 12).padding(.bottom, 10)
			.task { await model.chat.loadModelSuggestions() }
		}
		.background(.windowBackground)
		.accessibilityElement(children: .contain)
		.accessibilityIdentifier(Self.layerName)
		.task(id: model.chat.chat?.phase) {
			guard let phase = model.chat.chat?.phase, phase != .opening else { return }
			await model.chat.prepareHistory()
			model.prepared = true
		}
	}
}

/// AppKit's plain multi-line editor, with explicit send chords and IME handling.
struct AgentPrompt: NSViewRepresentable {
	@Binding var text: String
	let enterSends: Bool
	let onSend: () -> Void
	let onView: (NSTextView) -> Void

	func makeCoordinator() -> Coordinator { Coordinator(self) }
	func makeNSView(context: Context) -> NSScrollView {
		let scroll = NSScrollView()
		scroll.hasVerticalScroller = true
		let editor = PromptTextView()
		editor.isRichText = false
		editor.isAutomaticQuoteSubstitutionEnabled = false
		editor.isAutomaticDashSubstitutionEnabled = false
		editor.font = .systemFont(ofSize: 14)
		editor.textColor = .textColor
		editor.backgroundColor = .textBackgroundColor
		editor.autoresizingMask = [.width]
		editor.isVerticallyResizable = true
		editor.isHorizontallyResizable = false
		editor.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
		editor.textContainer?.containerSize = NSSize(width: 0, height: CGFloat.greatestFiniteMagnitude)
		editor.textContainer?.widthTracksTextView = true
		editor.delegate = context.coordinator
		editor.setAccessibilityIdentifier("agent-prompt")
		scroll.documentView = editor
		onView(editor)
		return scroll
	}
	func updateNSView(_ scroll: NSScrollView, context: Context) {
		context.coordinator.parent = self
		guard let editor = scroll.documentView as? PromptTextView else { return }
		if editor.string != text, !editor.hasMarkedText() { editor.string = text }
		editor.enterSends = enterSends
		editor.onSend = onSend
	}
	@MainActor final class Coordinator: NSObject, NSTextViewDelegate {
		var parent: AgentPrompt
		init(_ parent: AgentPrompt) { self.parent = parent }
		func textDidChange(_ notification: Notification) {
			guard let editor = notification.object as? NSTextView else { return }
			parent.text = editor.string
		}
	}
}

final class PromptTextView: NSTextView {
	var enterSends = false
	var onSend: (() -> Void)?
	func send(_ key: Layers.KeyPress) -> Bool {
		let enter = key.keyCode == KeyCode.returnKey || key.keyCode == KeyCode.keypadEnter
		guard enter, !hasMarkedText(),
			key.modifiers == .command || key.modifiers == .control || (enterSends && key.modifiers.isEmpty)
		else { return false }
		onSend?()
		return true
	}

	override func keyDown(with event: NSEvent) {
		if !send(Layers.KeyPress(event)) { super.keyDown(with: event) }
	}
}

/// The row below the prompt, with content-sized widgets and the current turn action on the right.
struct AgentWidgetBar: View {
	let context: ChatContext?
	let model: AgentModelPresentation
	let modelNames: [String]
	let enterSends: Bool
	let showsSystem: Bool
	let onShowsSystem: (Bool) -> Void
	let pause: AgentPausePresentation
	let compaction: AgentContextPresentation
	let turnRunning: Bool
	let canSend: Bool
	let onSend: () -> Void
	let onStop: () -> Void
	let onPause: () -> Void
	let onResume: () -> Void
	let onKeepPaused: (Bool) -> Void
	let onThreshold: (UInt8?) -> Void
	let onCompactNow: () -> Void

	var sendHint: String {
		enterSends ? "Enter sends · Shift+Enter newline" : "⌘Enter / Ctrl+Enter sends · Enter newline"
	}

	var body: some View {
		HStack(spacing: 12) {
			AgentContextWidget(context: context)
				.popover(compaction, onThreshold: onThreshold, onCompactNow: onCompactNow)
			AgentModelWidget(presentation: model, names: modelNames)
			AgentSystemMessagesToggle(shows: showsSystem, onChange: onShowsSystem)
			AgentPauseWidget(presentation: pause, onPause: onPause, onResume: onResume, onKeepPaused: onKeepPaused)
			Spacer(minLength: 12)
			if turnRunning {
				Button("■ Stop", action: onStop)
					.help("Interrupt the current turn")
					.accessibilityIdentifier("agent-stop")
			} else {
				Button("Send", action: onSend).disabled(!canSend)
					.help(sendHint)
					.accessibilityHint(sendHint)
			}
		}
	}
}

/// Shows or hides the chat's system messages (tool calls, thinking, the session's notes): their glyph and a native
/// switch, held together in a capsule, with what it does in the tooltip. What needs the user (errors, permission
/// requests) shows either way.
struct AgentSystemMessagesToggle: View {
	let shows: Bool
	let onChange: (Bool) -> Void

	static func help(_ shows: Bool) -> String {
		shows
			? "System messages: shown. Hide tool calls, thinking and the session's notes"
			: "System messages: hidden. Show tool calls, thinking and the session's notes"
	}

	var body: some View {
		HStack(spacing: 6) {
			Image(systemName: "text.bubble").foregroundStyle(.secondary).accessibilityHidden(true)
			Toggle("System messages", isOn: Binding(get: { shows }, set: onChange))
				.toggleStyle(.switch)
				.controlSize(.mini)
				.labelsHidden()
		}
		.padding(.horizontal, 8).padding(.vertical, 3)
		.background(.quaternary, in: Capsule())
		.overlay(Capsule().strokeBorder(Color(nsColor: .separatorColor), lineWidth: 1))
		.help(Self.help(shows))
		.accessibilityElement(children: .combine)
		.accessibilityLabel("System messages")
		.accessibilityValue(shows ? "shown" : "hidden")
		.accessibilityIdentifier("agent-system-messages")
	}
}

/// The context usage and the compaction threshold reported by the space actor, as a percentage and a ring, with the full numbers in the tooltip.
struct AgentContextWidget: View {
	let context: ChatContext?
	/// Formats the percentage and the token counts alike; the user's, following changes to their settings.
	var locale: Locale = .autoupdatingCurrent

	/// The used share of the window, at most 1; nil while the usage or the window size is unknown.
	var fraction: Double? {
		guard let context, context.windowTokens > 0 else { return nil }
		return min(1, Double(context.usedTokens) / Double(context.windowTokens))
	}

	/// How far the usage has come to the compaction threshold, which fills the ring; 0 while it is unknown.
	var ringFraction: Double {
		guard let fraction, let context, context.thresholdPercent > 0 else { return fraction ?? 0 }
		return min(1, fraction * 100 / Double(context.thresholdPercent))
	}

	/// The share in the user's format, with the % sign right after it, as in "15.1%" (some locales put a space before it).
	func percent(_ fraction: Double) -> String {
		"\((fraction * 100).formatted(.number.precision(.fractionLength(1)).locale(locale)))%"
	}

	var text: String {
		guard let fraction else { return "Context unknown" }
		return "Context \(percent(fraction))"
	}

	/// The usage, and the window once it's known.
	var tokens: String {
		guard let context else { return "Waiting for context usage" }
		let used = context.usedTokens.formatted(.number.locale(locale))
		guard context.windowTokens > 0 else { return "\(used) tokens · window size not known yet" }
		return "\(used) / \(context.windowTokens.formatted(.number.locale(locale))) tokens"
	}

	/// Whether this agent has a compaction threshold of its own stored, over its profile's, even an equal one.
	var overridden: Bool { context?.ownThresholdPercent != nil }

	var tooltip: String {
		guard let context else { return tokens }
		guard context.windowTokens > 0 else { return tokens }
		let threshold = overridden
			? "compaction at \(context.thresholdPercent)% for this agent (profile: \(context.defaultThresholdPercent)%)"
			: "compaction at \(context.thresholdPercent)%"
		return "\(tokens) · \(threshold)"
	}

	var body: some View {
		// A constant width: the text reserves its widest forms and the glyph its place, so the widget never moves the
		// context popover anchored to it (a moved anchor re-presents the popover) nor the Pause widget beside it.
		HStack(spacing: 6) {
			ZStack(alignment: .leading) {
				Text("Context \(percent(1))").hidden()
				Text("Context unknown").hidden()
				Text(text)
			}
			.monospacedDigit()
			ContextRing(fraction: ringFraction).frame(width: 20, height: 20)
			Image(systemName: "slider.horizontal.2.square").foregroundStyle(.secondary)
				.opacity(overridden ? 1 : 0)
				.accessibilityHidden(true)
		}
		.font(.callout)
		.foregroundStyle(fraction == nil ? .secondary : .primary)
		.fixedSize()
		.help(tooltip)
		.accessibilityElement(children: .combine)
		.accessibilityLabel("Agent context")
		.accessibilityValue(tooltip)
		.accessibilityIdentifier("agent-context")
	}
}

extension AgentContextWidget {
	/// Opens the context popover on a click, as a plain button around the widget.
	func popover(
		_ presentation: AgentContextPresentation, onThreshold: @escaping (UInt8?) -> Void,
		onCompactNow: @escaping () -> Void
	) -> some View {
		ContextButton(widget: self, presentation: presentation, onThreshold: onThreshold, onCompactNow: onCompactNow)
	}
}

private struct ContextButton: View {
	let widget: AgentContextWidget
	let presentation: AgentContextPresentation
	let onThreshold: (UInt8?) -> Void
	let onCompactNow: () -> Void
	@State private var shown = false

	var body: some View {
		Button { shown.toggle() } label: { widget }
			.buttonStyle(.plain)
			.popover(isPresented: $shown, arrowEdge: .top) {
				AgentContextPopover(
					widget: widget, presentation: presentation, onThreshold: onThreshold, onCompactNow: onCompactNow)
			}
	}
}

/// A ring filling clockwise from 9 o'clock (a circle's trim starts at 3 o'clock, hence the half turn).
struct ContextRing: View {
	let fraction: Double
	var lineWidth: CGFloat = 3

	var body: some View {
		ZStack {
			Circle().stroke(.quaternary, lineWidth: lineWidth)
			Circle().trim(from: 0, to: fraction).stroke(.tint, lineWidth: lineWidth)
		}
		.rotationEffect(.degrees(180))
		.padding(lineWidth / 2)
	}
}
