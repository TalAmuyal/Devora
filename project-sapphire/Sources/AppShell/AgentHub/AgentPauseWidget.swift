import CoreBindings
import SwiftUI

/// The chat projection's pause controls, including the distinction between a global and an individual hold.
struct AgentPausePresentation {
	enum Action: Equatable { case pause, resume, none }

	let phase: ChatPhase?
	let pauseRequested: Bool
	let globallyPaused: Bool
	let globallyHeld: Bool
	var keepPausedSelected = false

	var label: String {
		if globallyPaused {
			return globallyHeld && phase != .paused && phase != .failed ? "Pausing…" : "Paused"
		}
		if phase == .paused { return "Resume" }
		return isPausing ? "Pausing…" : "Pause"
	}

	var action: Action {
		if globallyPaused { return .none }
		if phase == .paused { return .resume }
		return phase == .running && !pauseRequested ? .pause : .none
	}

	var keepPaused: Bool { globallyPaused && globallyHeld }
	var keptPaused: Bool { globallyPaused && (keepPausedSelected || (!globallyHeld && (phase == .paused || isPausing))) }
	var isPausing: Bool { pauseRequested || phase == .pausing }
	var icon: String { phase == .paused && !globallyPaused ? "play.fill" : "pause.fill" }

	var accessibilityLabel: String {
		if globallyPaused { return "Devora paused" }
		if phase == .paused { return "Resume agent" }
		return isPausing ? "Pausing after this turn" : "Pause agent"
	}

	var tooltip: String {
		if globallyPaused { return "Resume Devora to send" }
		if phase == .paused { return "Resume this agent" }
		return isPausing ? "Pausing after the current turn ends" : "Pause after the current turn"
	}

	var accessibilityValue: String {
		if globallyPaused && globallyHeld && phase != .paused && phase != .failed { return "Pausing after this turn" }
		if keptPaused { return "Agent kept paused when Devora resumes" }
		return tooltip
	}
}

struct AgentPauseWidget: View {
	let presentation: AgentPausePresentation
	let onPause: () -> Void
	let onResume: () -> Void
	let onKeepPaused: (Bool) -> Void

	var body: some View {
		HStack(spacing: 8) {
			Group {
				if presentation.globallyPaused {
					Text(presentation.label).foregroundStyle(.secondary)
						.padding(.horizontal, 8).padding(.vertical, 3)
						.background(.quaternary, in: Capsule())
						.overlay(Capsule().strokeBorder(Color(nsColor: .separatorColor), lineWidth: 1))
				} else {
					Button {
						switch presentation.action {
						case .pause: onPause()
						case .resume: onResume()
						case .none: break
						}
					} label: {
						Label(presentation.label, systemImage: presentation.icon)
					}
					.buttonStyle(.bordered)
					.disabled(presentation.action == .none)
				}
			}
			.accessibilityLabel(presentation.accessibilityLabel)
			.accessibilityValue(presentation.accessibilityValue)
			.accessibilityIdentifier(presentation.globallyPaused ? "agent-global-pause-status" : "agent-pause-control")
			.help(presentation.tooltip)
			if presentation.keepPaused || presentation.keptPaused {
				Toggle("Keep paused", isOn: Binding(get: { presentation.keptPaused }, set: onKeepPaused))
					.toggleStyle(.switch)
					.disabled(!presentation.globallyHeld || presentation.phase == .failed)
					.help(presentation.globallyHeld
						? "Keep this agent paused when Devora resumes"
						: "This agent was individually paused. Resume it after Devora resumes.")
					.accessibilityIdentifier("agent-keep-paused")
			}
		}
		.controlSize(.small)
		.font(.callout)
		.fixedSize()
	}
}
