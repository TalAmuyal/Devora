import CoreBindings
import Foundation
import SwiftUI

/// A question the agent asks (a tool waiting for permission), as the card the host draws for it, with the chat's
/// generation that its answer must carry. Built only from `AgentChat.questions`, never from an item's content, so
/// nothing the agent writes can make one.
struct PermissionCard: Equatable {
	let question: PermissionQuestion
	let generation: UInt64

	var reason: String { question.decisionReason ?? "Permission requested" }

	/// The suggested rules as sentences ("Allow Bash(custom-tool), saved to localSettings"), or nil when there are none.
	/// An update this doesn't know, or one with a field it doesn't show, is its pretty-printed JSON, and JSON that isn't
	/// a list of updates is as it came, so nothing is lost.
	var rules: [String]? {
		guard let json = question.suggestedRulesJson, !json.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
		guard let updates = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [Any] else { return [prettyJSON(json)] }
		guard !updates.isEmpty else { return nil }
		return updates.map { update in
			(update as? [String: Any]).flatMap(Self.sentence) ?? Self.pretty(update)
		}
	}

	/// The card's text, as the bridge reads it and the web card's text has it.
	var plainText: String {
		[question.toolName, prettyJSON(question.inputJson), reason, rules?.joined(separator: "\n")]
			.compactMap { $0 }.joined(separator: "\n")
	}

	private static func sentence(_ update: [String: Any]) -> String? {
		guard let type = update["type"] as? String, let destination = update["destination"] as? String else { return nil }
		let text: String
		var fields: Set<String> = ["type", "destination"]
		switch type {
		case "addRules", "replaceRules", "removeRules":
			guard let behavior = update["behavior"] as? String, let rules = update["rules"] as? [Any] else { return nil }
			let names = rules.map { ($0 as? [String: Any]).flatMap(rule) }
			guard !names.isEmpty, names.allSatisfy({ $0 != nil }) else { return nil }
			let list = names.compactMap { $0 }.joined(separator: ", ")
			fields.formUnion(["behavior", "rules"])
			switch type {
			case "addRules": text = "\(verb(behavior)) \(list)"
			case "replaceRules": text = "Replace the \(behavior) rules with \(list)"
			default: text = "Remove the \(behavior) \(names.count == 1 ? "rule" : "rules") \(list)"
			}
		case "setMode":
			guard let mode = update["mode"] as? String else { return nil }
			fields.insert("mode")
			text = "Switch to the \(mode) mode"
		case "addDirectories", "removeDirectories":
			guard let directories = update["directories"] as? [String], !directories.isEmpty else { return nil }
			fields.insert("directories")
			text = "\(type == "addDirectories" ? "Add" : "Remove") \(directories.joined(separator: ", "))"
		default:
			return nil
		}
		guard Set(update.keys).isSubset(of: fields) else { return nil }
		return "\(text), saved to \(destination)"
	}

	/// A rule as Claude Code writes it in its settings: the tool, and its content in parentheses.
	private static func rule(_ rule: [String: Any]) -> String? {
		guard let tool = rule["toolName"] as? String, Set(rule.keys).isSubset(of: ["toolName", "ruleContent"]) else { return nil }
		guard let content = rule["ruleContent"] else { return tool }
		return (content as? String).map { "\(tool)(\($0))" }
	}

	private static func verb(_ behavior: String) -> String {
		switch behavior {
		case "allow": "Allow"
		case "deny": "Deny"
		case "ask": "Ask before"
		default: behavior
		}
	}

	private static func pretty(_ value: Any) -> String {
		guard let data = try? JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys, .fragmentsAllowed])
		else { return String(describing: value) }
		return String(decoding: data, as: UTF8.self)
	}
}

/// The web transcript's permission card: the tool, its input, the reason, the suggested rules (their JSON as the
/// tooltip), and the answers, whose keys the tab's layer handles from the editor or the transcript.
struct PermissionCardView: View {
	let card: PermissionCard
	/// Answers the question: "allow", "deny" or "always".
	var answer: (String) -> Void = { _ in }
	@Environment(\.transcriptTheme) private var theme

	var body: some View {
		VStack(alignment: .leading, spacing: 6) {
			Text(card.question.toolName).bold()
			Text(prettyJSON(card.question.inputJson)).font(.system(.callout, design: .monospaced))
			Text(card.reason).foregroundStyle(theme.secondary)
			if let rules = card.rules {
				VStack(alignment: .leading, spacing: 2) {
					Text(card.question.canAlwaysAllow ? "Always allow saves" : "Suggested").font(.caption).foregroundStyle(theme.secondary)
					ForEach(rules.indices, id: \.self) { index in Text(rules[index]) }
				}
				.help(card.question.suggestedRulesJson.map(prettyJSON) ?? "")
			}
			HStack(spacing: 8) {
				Button("Allow ⌘Y") { answer("allow") }
				Button("Deny ⌘N") { answer("deny") }
				if card.question.canAlwaysAllow { Button("Always allow ⌘⇧Y") { answer("always") } }
			}
			.padding(.top, 2)
		}
		.textSelection(.enabled)
		.frame(maxWidth: .infinity, alignment: .leading)
		.padding(10)
		.background(Self.amber.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
		.overlay(RoundedRectangle(cornerRadius: 8).stroke(Self.amber.opacity(0.6)))
		.padding(.leading, 22)
		.padding(.horizontal, 16).padding(.vertical, 6)
		.accessibilityElement(children: .contain)
		.accessibilityLabel("Permission request")
	}

	private static let amber = Color(red: 0.96, green: 0.62, blue: 0.04)
}
