import CoreBindings
import Testing

@testable import AppShell

@MainActor
struct AgentEffortTests {
	let all = ["low", "medium", "high", "xhigh", "max"]

	func effort(
		_ setTo: String?, levels: [String]?, modelUnknown: Bool = false, runsAs: String? = nil, pending: Bool = false,
		notTaken: String? = nil
	) -> ChatEffort {
		ChatEffort(
			setTo: setTo, levels: levels, modelUnknown: modelUnknown, runsAs: runsAs, pending: pending, notTaken: notTaken)
	}

	func shown(
		_ effort: ChatEffort?, phase: ChatPhase = .running, turn: Bool = false, globallyPaused: Bool = false
	) -> AgentEffortPresentation {
		AgentEffortPresentation(effort: effort, phase: phase, turnRunning: turn, globallyPaused: globallyPaused)
	}

	@Test func anIdleAgentShowsItsLevelAndOffersItsModelsLevelsWithTheStoredOneChecked() {
		let idle = shown(effort("high", levels: all))
		#expect(idle.label == "high" && !idle.marked && !idle.grayed && idle.enabled)
		#expect(idle.levels == all && idle.setTo == "high" && idle.note == nil)
		#expect(shown(effort(nil, levels: all)).label == "default", "Claude Code's default reads default")
	}

	@Test func aLevelPickedDuringATurnIsOrangeUntilItApplies() {
		#expect(shown(effort("high", levels: all), turn: true).note == "A turn is running: a new level applies when it ends.")
		let pending = shown(effort("max", levels: all, pending: true), turn: true)
		#expect(pending.label == "max" && pending.marked)
		#expect(pending.note == "max applies when this turn ends.")
	}

	@Test func aLevelTheModelLacksShowsWhatRunsMarkedWithTheStoredOneSaid() {
		let lacks = shown(effort("xhigh", levels: ["low", "medium", "high", "max"], runsAs: "high"))
		#expect(lacks.label == "high" && lacks.marked)
		#expect(lacks.note == "This model runs xhigh as high.")
		#expect(lacks.setTo == "xhigh", "the stored level is kept")
		#expect(lacks.accessibilityValue == "high, This model runs xhigh as high.")
	}

	@Test func aModelThatTakesNoEffortShowsADashGrayedAndOffersNothing() {
		let none = shown(effort("high", levels: []))
		#expect(none.label == "—" && none.grayed && !none.marked && !none.enabled)
		#expect(none.note == "This model takes no effort level.")
	}

	@Test func nothingCanBePickedUntilTheLevelsAreKnownOrWhileStartingOrPausing() {
		let unknown = shown(effort("high", levels: nil))
		#expect(unknown.label == "high" && !unknown.enabled)
		#expect(unknown.note == "Waiting for the levels Claude Code offers")
		#expect(!shown(effort("high", levels: all), phase: .pausing).enabled)
		#expect(shown(effort("high", levels: all), phase: .opening).note == "This agent is starting")
		let paused = shown(effort("high", levels: all), phase: .paused)
		#expect(paused.enabled && paused.grayed, "a paused agent's pick is stored for its next launch")
		#expect(paused.note == "Paused: the level you pick applies when the agent resumes.")
	}

	@Test func aModelTheListDoesntKnowSaysSoAndOffersOnlyClaudeCodesDefault() {
		let unknown = shown(effort("high", levels: nil, modelUnknown: true))
		#expect(unknown.label == "high" && !unknown.enabled && unknown.defaultEnabled)
		#expect(unknown.note == "Claude Code doesn't list this model, so its effort levels aren't known.")
		let waiting = shown(effort("high", levels: nil))
		#expect(!waiting.defaultEnabled, "nothing can be picked while the list is coming")
	}

	@Test func aRefusedLevelIsMarkedAndSaysWhy() {
		let refused = shown(effort("low", levels: all, notTaken: "effort refused"))
		#expect(refused.marked)
		#expect(refused.note == "Claude Code didn't take low (effort refused). It's tried again at the next launch.")
	}
}
