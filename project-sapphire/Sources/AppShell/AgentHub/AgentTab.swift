import AppKit
import Store
import SwiftUI

/// An agent's tab until its chat panel lands: its name and status, on a page at the bottom of the tab's stack.
struct AgentTabPage: View {
	static let layerName = "agent-tab"

	let store: Store
	let tab: UInt64
	let name: String

	var body: some View {
		let status = store.agentStatuses[tab]
		ContentUnavailableView {
			HStack(spacing: 8) {
				if let status { AgentStatusEmoji(status: status) }
				Text(name)
			}
			.font(.title2)
		} description: {
			Text(status.map(Self.describe) ?? "Starting…")
			Text("The chat panel is coming in a later release.").foregroundStyle(.tertiary)
		}
		.frame(maxWidth: .infinity, maxHeight: .infinity)
		.background(.windowBackground)
		.accessibilityIdentifier(Self.layerName)
	}

	static func describe(_ status: AgentStatus) -> String {
		switch status {
		case .working: "Working"
		case .waiting: "Waiting for you"
		case .idle: "Idle"
		case .pausing: "Pausing"
		case .failed: "Failed"
		}
	}
}

/// An agent's status as its tab shows it (ADR-012): ⚙️ turning slowly while it works, ❓ when it waits for the user, 💤 idle, ⏸️ pausing, ⚠️ failed.
/// The cog stands still under Reduce Motion and while the window isn't active.
struct AgentStatusEmoji: View {
	let status: AgentStatus
	@Environment(\.accessibilityReduceMotion) private var reduceMotion
	@Environment(\.appearsActive) private var appearsActive

	var body: some View {
		Text(Self.emoji(status))
			.rotationEffect(.degrees(isTurning ? 360 : 0))
			.animation(isTurning ? .linear(duration: 4).repeatForever(autoreverses: false) : .default, value: isTurning)
			.help(AgentTabPage.describe(status))
			.accessibilityLabel(AgentTabPage.describe(status))
	}

	private var isTurning: Bool { status == .working && !reduceMotion && appearsActive }

	static func emoji(_ status: AgentStatus) -> String {
		switch status {
		case .working: "⚙️"
		case .waiting: "❓"
		case .idle: "💤"
		case .pausing: "⏸️"
		case .failed: "⚠️"
		}
	}
}
