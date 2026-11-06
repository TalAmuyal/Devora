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
					"kind": "tool", "text": "", "name": name, "input": pretty(input),
					"result": result?.text ?? "Awaiting result…",
					"verdict": verdict?.kind ?? "",
					"verdictDetail": verdict?.detail ?? "",
				]) { _, new in new }
			case .system(let kind, let text, _):
				row.merge(["kind": "system", "text": text.isEmpty ? String(describing: kind) : text]) {
					_, new in new
				}
			case .unknown(let json): row.merge(["kind": "system", "text": pretty(json)]) { _, new in new }
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
				"input": pretty(question.inputJson),
				"reason": question.decisionReason ?? "Permission requested",
				"rules": question.suggestedRulesJson.map(pretty) ?? "",
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

	private func pretty(_ value: String) -> String {
		guard let data = value.data(using: .utf8),
			let json = try? JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed),
			let formatted = try? JSONSerialization.data(
				withJSONObject: json, options: [.prettyPrinted, .sortedKeys, .fragmentsAllowed])
		else { return value }
		return String(decoding: formatted, as: UTF8.self)
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
				if model.chat.chat?.phase == .paused {
					Button("Resume") { Task { await model.chat.retry() } }
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
					Button("Retry") { Task { await model.chat.retry() } }
				}
				.padding(12).foregroundStyle(.orange)
			}
			if model.prepared {
				WebContentView(
					content: { ("", model.theme) },
					onError: { model.rendererError = Store.describe($0) },
					onView: { model.webView = $0 }, revision: "chat-\(model.chat.revision)",
					chat: model.payload, onMessage: model.message)
			} else {
				ProgressView("Opening agent…").frame(maxWidth: .infinity, maxHeight: .infinity)
			}
			Divider()
			// Bravo's context state supplies the context window and threshold after integration.
			Text("Context usage pending").font(.caption).foregroundStyle(.secondary).frame(
				maxWidth: .infinity, alignment: .leading
			).padding(.horizontal, 12).padding(.vertical, 6)
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
			HStack {
				Text(
					model.chat.enterSends
						? "Enter sends · Shift+Enter newline"
						: "⌘Enter / Ctrl+Enter sends · Enter newline"
				).font(.caption).foregroundStyle(.secondary)
				Spacer()
				Button("Send") { Task { await model.chat.send() } }.disabled(!model.chat.canSend)
			}.padding(.horizontal, 12).padding(.bottom, 10)
		}
		.background(.windowBackground)
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
