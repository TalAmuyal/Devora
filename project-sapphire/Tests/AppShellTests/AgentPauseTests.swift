import CoreBindings
import Testing

@testable import AppShell

@MainActor
struct AgentPauseTests {
	typealias Action = AgentPausePresentation.Action

	@Test func individualPauseStates() {
		let cases: [(ChatPhase?, Bool, String, Action, String)] = [
			(nil, false, "Pause", .none, "Pause agent"),
			(nil, true, "Pausing…", .none, "Pausing after this turn"),
			(.opening, false, "Pause", .none, "Pause agent"),
			(.opening, true, "Pausing…", .none, "Pausing after this turn"),
			(.running, false, "Pause", .pause, "Pause agent"),
			(.running, true, "Pausing…", .none, "Pausing after this turn"),
			(.pausing, false, "Pausing…", .none, "Pausing after this turn"),
			(.pausing, true, "Pausing…", .none, "Pausing after this turn"),
			(.paused, false, "Resume", .resume, "Resume agent"),
			(.paused, true, "Resume", .resume, "Resume agent"),
			(.failed, false, "Pause", .none, "Pause agent"),
			(.failed, true, "Pausing…", .none, "Pausing after this turn"),
		]
		for (phase, requested, label, action, accessibility) in cases {
			for held in [false, true] {
				let state = AgentPausePresentation(
					phase: phase, pauseRequested: requested, globallyPaused: false, globallyHeld: held)
				#expect(state.label == label)
				#expect(state.action == action)
				#expect(state.accessibilityLabel == accessibility)
				#expect(!state.keepPaused)
				#expect(!state.keptPaused)
			}
		}
	}

	@Test func globalHoldsDrainBeforeShowingDevoraPaused() {
		let cases: [(ChatPhase?, String, String)] = [
			(nil, "Pausing…", "Pausing after this turn"),
			(.opening, "Pausing…", "Pausing after this turn"),
			(.running, "Pausing…", "Pausing after this turn"),
			(.pausing, "Pausing…", "Pausing after this turn"),
			(.paused, "Paused", "Resume Devora to send"),
			(.failed, "Paused", "Resume Devora to send"),
		]
		for (phase, label, value) in cases {
			for requested in [false, true] {
				let state = AgentPausePresentation(
					phase: phase, pauseRequested: requested, globallyPaused: true, globallyHeld: true)
				#expect(state.label == label)
				#expect(state.action == .none)
				#expect(state.keepPaused)
				#expect(!state.keptPaused)
				#expect(state.accessibilityLabel == "Devora paused")
				#expect(state.accessibilityValue == value)
				#expect(state.tooltip == "Resume Devora to send")
			}
		}
	}

	@Test func individuallyHeldAgentsStayHeldDuringGlobalPause() {
		let cases: [(ChatPhase?, Bool, Bool)] = [
			(nil, false, false), (nil, true, true),
			(.opening, false, false), (.opening, true, true),
			(.running, false, false), (.running, true, true),
			(.pausing, false, true), (.pausing, true, true),
			(.paused, false, true), (.paused, true, true),
			(.failed, false, false), (.failed, true, true),
		]
		for (phase, requested, kept) in cases {
			let state = AgentPausePresentation(
				phase: phase, pauseRequested: requested, globallyPaused: true, globallyHeld: false)
			#expect(state.label == "Paused")
			#expect(state.action == .none)
			#expect(!state.keepPaused)
			#expect(state.keptPaused == kept)
			#expect(state.tooltip == "Resume Devora to send")
		}
	}

	@Test func keepChoiceChangesWithoutConvertingTheGlobalHold() {
		for selected in [false, true, false] {
			let state = AgentPausePresentation(
				phase: .paused, pauseRequested: false, globallyPaused: true, globallyHeld: true,
				keepPausedSelected: selected)
			#expect(state.keepPaused)
			#expect(state.keptPaused == selected)
			#expect(state.label == "Paused")
			#expect(state.action == .none)
		}
		let draining = AgentPausePresentation(
			phase: .running, pauseRequested: true, globallyPaused: true, globallyHeld: true,
			keepPausedSelected: true)
		#expect(draining.label == "Pausing…")
		#expect(draining.accessibilityValue == "Pausing after this turn")
	}

	@Test func keepingPausedThenResumingDevoraOffersOnlyIndividualResume() {
		let globalHold = AgentPausePresentation(
			phase: .paused, pauseRequested: false, globallyPaused: true, globallyHeld: true)
		#expect(globalHold.keepPaused)
		let kept = AgentPausePresentation(
			phase: .paused, pauseRequested: false, globallyPaused: true, globallyHeld: false)
		#expect(kept.keptPaused)
		#expect(kept.accessibilityValue == "Agent kept paused when Devora resumes")
		#expect(kept.action == .none)
		let released = AgentPausePresentation(
			phase: .paused, pauseRequested: false, globallyPaused: false, globallyHeld: false)
		#expect(released.label == "Resume")
		#expect(released.action == .resume)
		#expect(!released.keepPaused)
		#expect(released.tooltip == "Resume this agent")
	}
}
