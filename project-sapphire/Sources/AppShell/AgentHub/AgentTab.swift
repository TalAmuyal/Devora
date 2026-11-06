import AppKit
import Store
import SwiftUI

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
			.help(Self.describe(status))
			.accessibilityLabel(Self.describe(status))
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
