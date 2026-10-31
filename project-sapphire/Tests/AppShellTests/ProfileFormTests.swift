@testable import AppShell
import CoreBindings
import Layers
import Testing

/// The profile form's validation and keys, as Ember's `ProfileForm` behaves.
@MainActor
struct ProfileFormTests {
	final class Calls {
		var validated: [String] = []
		var registered: [(path: String, name: String)] = []
		var cancelled = false
	}

	let calls = Calls()
	let enter = KeyPress(keyCode: KeyCode.returnKey, characters: "\r")
	let space = KeyPress(keyCode: KeyCode.space, characters: " ")

	/// A form whose validation classifies a path by its last component: `legacy` is an existing profile, `bad` is invalid, anything else is new.
	func form(initialPath: String = "", hasCancel: Bool = false, delay: Duration = .milliseconds(30)) -> ProfileFormModel {
		let calls = calls
		return ProfileFormModel(
			initialPath: initialPath, hasCancel: hasCancel,
			validate: { path in
				calls.validated.append(path)
				switch path.split(separator: "/").last {
				case "legacy": return .existingProfile(expandedPath: path, name: "Legacy")
				case "bad": return .invalid(expandedPath: path, error: "Parent directory does not exist")
				default: return .new(expandedPath: path)
				}
			},
			register: { path, name in
				calls.registered.append((path, name))
				return true
			},
			browseForFolder: { "/picked/legacy" },
			onCancel: { calls.cancelled = true },
			clearFocus: {},
			validationDelay: delay)
	}

	@Test func editsAreValidatedOnceAfterThePauseThatFollowsTheLast() async throws {
		let model = form()

		model.editPath("/a")
		model.editPath("/ab")
		model.editPath("/abc")
		#expect(model.validating && !model.canSubmit)
		try await eventually { !model.validating }

		#expect(calls.validated == ["/abc"])
		#expect(model.status == .new)
	}

	@Test func anInitialOrBrowsedPathIsValidatedAtOnce() async throws {
		let model = form(initialPath: "~/devora", delay: .seconds(60))
		try await eventually { !model.validating }
		#expect(model.status == .new)

		model.browse()
		try await eventually { model.status == .existing(name: "Legacy") }

		#expect(calls.validated == ["~/devora", "/picked/legacy"])
	}

	@Test func anExistingProfileLocksTheNameUntilThePathChangesAgain() async throws {
		let model = form()
		model.typedName = "Mine"

		model.editPath("/x/legacy")
		try await eventually { !model.validating }
		#expect(model.isNameLocked && model.name == "Legacy")
		#expect(model.submitLabel == "Register Profile")

		model.editPath("/x/fresh")
		try await eventually { !model.validating }
		#expect(!model.isNameLocked && model.name == "Mine")
		#expect(model.submitLabel == "Create Profile")
	}

	@Test func aNewProfileNeedsANameAndAnInvalidPathCantBeSubmitted() async throws {
		let model = form()
		model.editPath("/x/fresh")
		try await eventually { !model.validating }
		#expect(!model.canSubmit, "a new profile needs a name")
		model.typedName = "  "
		#expect(!model.canSubmit)
		model.typedName = " Work "
		#expect(model.canSubmit)

		model.editPath("/x/bad")
		try await eventually { !model.validating }
		#expect(model.status == .invalid("Parent directory does not exist"))
		#expect(!model.canSubmit)

		model.editPath("")
		try await eventually { !model.validating }
		#expect(model.status == nil, "a cleared path isn't scolded")
	}

	@Test func enterInAFieldSubmitsTheTrimmedName() async throws {
		let model = form()
		model.typedName = " Work "
		model.editPath("/x/work")
		try await eventually { model.canSubmit }

		model.focus = .path
		#expect(model.handleKey(enter))
		try await eventually { !calls.registered.isEmpty }

		#expect(calls.registered.map(\.path) == ["/x/work"])
		#expect(calls.registered.map(\.name) == ["Work"])
	}

	@Test func withNothingFocusedEnterDoesNothing() async throws {
		let model = form()
		model.typedName = "Work"
		model.editPath("/x/work")
		try await eventually { model.canSubmit }

		#expect(model.handleKey(enter))
		try await Task.sleep(for: .milliseconds(50))

		#expect(calls.registered.isEmpty)
	}

	@Test func tabSkipsTheLockedNameAndTheDisabledSubmit() async throws {
		let model = form(hasCancel: true)
		model.cycleFocus(backward: false)
		#expect(model.focus == .name)
		model.cycleFocus(backward: false)
		model.cycleFocus(backward: false)
		#expect(model.focus == .browse)
		model.cycleFocus(backward: false)
		#expect(model.focus == .cancel, "submit is disabled without a validated path")

		model.editPath("/x/legacy")
		try await eventually { model.canSubmit }
		model.cycleFocus(backward: false)
		#expect(model.focus == .path, "the locked name is skipped")
		model.cycleFocus(backward: true)
		#expect(model.focus == .cancel)
		model.cycleFocus(backward: true)
		#expect(model.focus == .submit)
	}

	@Test func spacePressesTheFocusedButtonAndTypesInAField() {
		let model = form(hasCancel: true)
		model.focus = .name
		#expect(!model.handleKey(space), "typing goes to the field")

		model.focus = .cancel
		#expect(model.handleKey(space))
		#expect(calls.cancelled)
	}
}

/// Polls until `condition` holds, for the form's asynchronous validation.
@MainActor
private func eventually(_ condition: () -> Bool) async throws {
	for _ in 0..<200 {
		if condition() { return }
		try await Task.sleep(for: .milliseconds(10))
	}
	Issue.record("the condition never held")
}
