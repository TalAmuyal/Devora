import AppKit
import CoreBindings
import Layers
import Observation
import Store
import SwiftUI
import WebKit

@MainActor
@Observable
final class AgentPanelModel {
	let chat: AgentChat
	var prepared = false
	var rendererError: String?
	@ObservationIgnored weak var editor: NSTextView?
	@ObservationIgnored weak var webView: WKWebView?
	@ObservationIgnored weak var transcript: TranscriptScrollView?
	/// Whether this panel shows the native transcript (ADR-014) rather than the web one: the hidden
	/// `DevoraNativeTranscript` default, until the native transcript is the only one.
	let nativeTranscript = UserDefaults.standard.bool(forKey: "DevoraNativeTranscript")
	@ObservationIgnored var onEditorReady: (() -> Void)?
	@ObservationIgnored let theme: [String: String]

	init(chat: AgentChat, store: Store) {
		self.chat = chat
		theme = (try? store.userGuideContent(resourceDir: Bundle.main.resourcePath ?? "").1) ?? [:]
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

	func message(_ body: [String: Any]) {
		if let permission = body["permission"] as? [String: String], let id = permission["id"],
			let action = permission["action"], let raw = permission["generation"],
			let generation = UInt64(raw)
		{
			Task { await chat.answer(id, action: action, generation: generation) }
		}
		if body.keys.contains("scroll") {
			if let anchor = body["scroll"] as? [String: Any], let sequence = anchor["sequence"] as? String,
				let id = UInt64(sequence), let offset = anchor["offset"] as? Int32
			{
				chat.saveScroll(ChatScrollAnchor(sequence: id, offset: offset))
			} else if body["scroll"] is NSNull {
				chat.saveScroll(nil)
			}
		}
		if body["older"] as? Bool == true { Task { await chat.older() } }
	}

	func payload() -> [String: Any] {
		let items = chat.items.map { item -> [String: Any] in
			var row: [String: Any] = [
				"sequence": String(item.sequence), "state": String(describing: item.state),
			]
			switch item.content {
			case .user(let text, let error):
				row.merge(["kind": "user", "text": text + (error.map { "\n\n\($0)" } ?? "")]) {
					_, new in new
				}
			case .assistant(let text): row.merge(["kind": "assistant", "text": text]) { _, new in new }
			case .thinking(let text, _): row.merge(["kind": "thinking", "text": text]) { _, new in new }
			case .tool(_, let name, let input, let verdict, let result):
				row.merge([
					"kind": "tool", "text": "", "name": name, "input": prettyJSON(input),
					"result": result?.text ?? "Awaiting result…",
					"verdict": verdict?.kind ?? "",
					"verdictDetail": verdict?.detail ?? "",
				]) { _, new in new }
			case .system(let kind, let text, _):
				row.merge(["kind": "system", "text": text.isEmpty ? String(describing: kind) : text]) {
					_, new in new
				}
			case .unknown(let json): row.merge(["kind": "system", "text": prettyJSON(json)]) { _, new in new }
			}
			return row
		}
		let questions = (chat.chat?.questions ?? []).map { question -> [String: Any] in
			let sequence = chat.items.first { item in
				if case .tool(let id, _, _, _, _) = item.content { return id == question.toolUseId }
				return false
			}?.sequence
			var row: [String: Any] = [
				"id": question.requestId, "tool": question.toolName,
				"input": prettyJSON(question.inputJson),
				"reason": question.decisionReason ?? "Permission requested",
				"rules": question.suggestedRulesJson.map { prettyJSON($0) } ?? "",
				"always": question.canAlwaysAllow, "generation": String(chat.chat?.generation ?? 0),
			]
			if let sequence { row["sequence"] = String(sequence) }
			return row
		}
		let anchor: Any =
			chat.anchor.map { ["sequence": String($0.sequence), "offset": $0.offset] as [String: Any] }
			?? NSNull()
		return ["items": items, "questions": questions, "anchor": anchor, "hasOlder": chat.hasOlder]
	}
}

struct AgentChatPanel: View {
	static let layerName = "agent-chat"
	@Bindable var model: AgentPanelModel

	var body: some View {
		VStack(spacing: 0) {
			HStack {
				Text(model.chat.name).font(.headline)
				Spacer()
				if model.chat.chat?.turnRunning == true {
					Button("■ Stop") { Task { await model.chat.stop() } }
				}
				if model.chat.globallyPaused && model.chat.chat?.globallyHeld == true {
					Text(model.chat.chat?.phase == .paused ? "Paused" : "Pausing…").font(.caption)
						.accessibilityIdentifier("agent-global-pause-status")
					Button("Keep Paused") { model.chat.pause() }
				} else if model.chat.chat?.phase == .paused {
					Button("Resume") { Task { await model.chat.retry() } }.disabled(model.chat.globallyPaused)
				} else {
					Button(model.chat.chat?.pauseRequested == true ? "⏸️ Pausing…" : "Pause") {
						model.chat.pause()
					}
					.disabled(
						model.chat.chat?.phase != .running
							|| model.chat.chat?.pauseRequested == true)
				}
			}.padding(12)
			Divider()
			if let error = model.chat.chat?.error ?? model.chat.error ?? model.rendererError {
				HStack {
					Text(error).textSelection(.enabled)
					Spacer()
					Button("Retry") { Task { await model.chat.retry() } }.disabled(model.chat.globallyPaused)
				}
				.padding(12).foregroundStyle(.orange)
			}
			if model.prepared && model.nativeTranscript {
				TranscriptView(
					items: model.chat.items, hasOlder: model.chat.hasOlder, anchor: model.chat.anchor,
					onScroll: { model.chat.saveScroll($0) }, onNeedsOlder: { Task { await model.chat.older() } },
					onView: { model.transcript = $0 })
			} else if model.prepared {
				WebContentView(
					content: { ("", model.theme) },
					onError: { model.rendererError = Store.describe($0) },
					onView: { model.webView = $0 }, revision: "chat-\(model.chat.revision)",
					chat: model.payload, onMessage: model.message)
			} else {
				ProgressView("Opening agent…").frame(maxWidth: .infinity, maxHeight: .infinity)
			}
			Divider()
			if model.chat.globallyPaused {
				Text("Globally paused — resume Devora to send")
					.font(.caption).foregroundStyle(.secondary)
					.padding(.vertical, 4)
					.accessibilityIdentifier("global-pause-send-hint")
				Divider()
			}
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
				context: model.chat.chat?.context, enterSends: model.chat.enterSends,
				canSend: model.chat.canSend, onSend: { Task { await model.chat.send() } }
			)
			.padding(.horizontal, 12).padding(.bottom, 10)
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

/// The row below the prompt: the widgets on the left, in order and each at its natural size, and Send on the right.
struct AgentWidgetBar: View {
	let context: ChatContext?
	let enterSends: Bool
	let canSend: Bool
	let onSend: () -> Void

	var sendHint: String {
		enterSends ? "Enter sends · Shift+Enter newline" : "⌘Enter / Ctrl+Enter sends · Enter newline"
	}

	var body: some View {
		HStack(spacing: 12) {
			AgentContextWidget(context: context)
			Spacer(minLength: 12)
			Button("Send", action: onSend).disabled(!canSend)
				.help(sendHint)
				.accessibilityHint(sendHint)
		}
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

	var text: String {
		guard let fraction else { return "Context unknown" }
		// The number in the user's format, with the % sign right after it, as in "15.1%" (some locales put a space before it).
		let percent = (fraction * 100).formatted(.number.precision(.fractionLength(1)).locale(locale))
		return "Context \(percent)%"
	}

	var tooltip: String {
		guard let context else { return "Waiting for context usage" }
		let used = context.usedTokens.formatted(.number.locale(locale))
		guard context.windowTokens > 0 else { return "\(used) tokens · window size not known yet" }
		let window = context.windowTokens.formatted(.number.locale(locale))
		return "\(used) / \(window) tokens · compaction at \(context.thresholdPercent)%"
	}

	var body: some View {
		HStack(spacing: 6) {
			Text(text).monospacedDigit()
			ContextRing(fraction: ringFraction).frame(width: 20, height: 20)
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

/// A ring filling clockwise from 9 o'clock (a circle's trim starts at 3 o'clock, hence the half turn).
struct ContextRing: View {
	let fraction: Double
	private let lineWidth: CGFloat = 3

	var body: some View {
		ZStack {
			Circle().stroke(.quaternary, lineWidth: lineWidth)
			Circle().trim(from: 0, to: fraction).stroke(.tint, lineWidth: lineWidth)
		}
		.rotationEffect(.degrees(180))
		.padding(lineWidth / 2)
	}
}
