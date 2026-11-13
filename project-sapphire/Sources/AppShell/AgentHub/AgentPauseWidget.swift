import CoreBindings
import SwiftUI

/// The chat projection's pause controls, including the distinction between a global and an individual hold.
struct AgentPausePresentation {
	enum Action: Equatable { case pause, resume, none }

	let phase: ChatPhase?
	let pauseRequested: Bool
	let globallyPaused: Bool
	let globallyHeld: Bool

	var label: String {
		if globallyPaused {
			return globallyHeld && phase != .paused ? "Pausing…" : "Devora paused"
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
	var keptPaused: Bool { globallyPaused && !globallyHeld && (phase == .paused || isPausing) }
	var isPausing: Bool { pauseRequested || phase == .pausing }
	var isPaused: Bool { globallyPaused || phase == .paused || isPausing }
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
		if keptPaused { return "Agent kept paused when Devora resumes" }
		if globallyPaused && globallyHeld && phase != .paused { return "Pausing after this turn" }
		return tooltip
	}
}

struct AgentPauseWidget: View {
	let presentation: AgentPausePresentation
	let theme: TranscriptTheme
	let onPause: () -> Void
	let onResume: () -> Void

	var body: some View {
		HStack(spacing: 8) {
			Group {
				if presentation.globallyPaused {
					Label(presentation.label, systemImage: presentation.icon)
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
					.disabled(presentation.action == .none)
				}
			}
			.accessibilityLabel(presentation.accessibilityLabel)
			.accessibilityValue(presentation.accessibilityValue)
			.accessibilityIdentifier(presentation.globallyPaused ? "agent-global-pause-status" : "agent-pause-control")
			.foregroundStyle(presentation.isPaused ? theme.link : theme.text)
			.padding(.horizontal, 8).padding(.vertical, 4)
			.background {
				RoundedRectangle(cornerRadius: 6)
					.fill(Color(nsColor: theme.background ?? .controlBackgroundColor))
					.overlay {
						RoundedRectangle(cornerRadius: 6)
							.fill(presentation.isPaused ? theme.link.opacity(0.1) : .clear)
					}
			}
			if presentation.keepPaused || presentation.keptPaused {
				Button(action: onPause) {
					Label("Keep paused", systemImage: presentation.keptPaused ? "checkmark" : "pause.fill")
				}
				.buttonStyle(KeepPausedButtonStyle(theme: theme, selected: presentation.keptPaused))
				.disabled(presentation.keptPaused)
				.help("Keep this agent paused when Devora resumes")
				.accessibilityValue(presentation.keptPaused ? "On. Agent kept paused when Devora resumes" : "Off")
				.accessibilityAddTraits(presentation.keptPaused ? .isSelected : [])
				.accessibilityIdentifier("agent-keep-paused")
			}
		}
		.buttonStyle(.plain)
		.font(.callout)
		.fixedSize()
		.help(presentation.tooltip)
	}
}

/// A separate action surface that stays inset once the individual hold is chosen.
private struct KeepPausedButtonStyle: ButtonStyle {
	let theme: TranscriptTheme
	let selected: Bool

	func makeBody(configuration: Configuration) -> some View {
		let pressed = selected || configuration.isPressed
		configuration.label
			.foregroundStyle(pressed ? theme.link : theme.text)
			.padding(.horizontal, 8).padding(.vertical, 4)
			.background {
				RoundedRectangle(cornerRadius: 6)
					.fill(Color(nsColor: theme.background ?? .controlBackgroundColor)
						.shadow(.inner(color: .black.opacity(pressed ? 0.55 : 0), radius: 2, x: 0, y: 2)))
			}
			.overlay {
				RoundedRectangle(cornerRadius: 6)
					.stroke(pressed ? theme.link.opacity(0.7) : theme.border, lineWidth: 1)
			}
			.shadow(color: .black.opacity(pressed ? 0 : 0.2), radius: 1, x: 0, y: 1)
			.offset(y: pressed ? 1 : 0)
	}
}
