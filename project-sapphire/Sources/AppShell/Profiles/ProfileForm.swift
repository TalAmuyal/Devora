import AppKit
import CoreBindings
import Layers
import Store
import SwiftUI

/// Ember's profile form (`ProfileForm.ts`): a name and a root path, which it validates 250 ms after the last edit, as a new profile, an existing one, or an invalid path.
///
/// An existing profile locks the name to its own and turns "Create Profile" into "Register Profile"; the name typed before comes back when the path changes again.
/// It is in the first-run welcome (`WorkspaceHubModel.welcome`) and in the Settings Hub's New Profile row (`SettingsHubModel.form`), which handle Tab and Escape; it handles the other keys, and draws its buttons' focus itself, as the dialogs do.
@MainActor
@Observable
final class ProfileFormModel {
	static let validationDelay = Duration.milliseconds(250)

	/// In Tab order, which is the order they are drawn in.
	enum Focus: CaseIterable {
		case name
		case path
		case browse
		case submit
		case cancel

		var isTextField: Bool { self == .name || self == .path }
	}

	/// What the path is, once validated.
	enum Status: Equatable {
		case new
		case existing(name: String)
		case invalid(String)

		init(_ validation: ProfilePathValidation) {
			switch validation {
			case .new: self = .new
			case .existingProfile(_, let name): self = .existing(name: name)
			case .invalid(_, let error): self = .invalid(error)
			}
		}
	}

	/// The name typed, which an existing profile's own name hides without replacing.
	var typedName = ""
	/// Edited through `editPath`, which validates it after a pause.
	private(set) var path = ""
	/// Nil while the path is empty, and until its first validation.
	private(set) var status: Status?
	/// From an edit of the path until its validation is back.
	private(set) var validating = false
	private(set) var submitting = false
	/// Nil when none of the form's controls has the focus; giving it to a button takes it from the text fields.
	var focus: Focus? {
		didSet { if focus?.isTextField != true { clearFocus() } }
	}
	/// Whether the form has a Cancel button: the Settings Hub's has, the first-run welcome's hasn't.
	let hasCancel: Bool

	@ObservationIgnored private let validate: (String) async -> ProfilePathValidation?
	@ObservationIgnored private let register: (_ path: String, _ name: String) async -> Bool
	@ObservationIgnored private let browseForFolder: () async -> String?
	@ObservationIgnored private let onCancel: () -> Void
	@ObservationIgnored private let clearFocus: () -> Void
	@ObservationIgnored private let validationDelay: Duration
	@ObservationIgnored private var validation: Task<Void, Never>?

	/// `validate` and `register` are the core's calls, nil and false when they failed (with their banners); `register` closes the form when it succeeds.
	init(
		initialPath: String = "", hasCancel: Bool,
		validate: @escaping (String) async -> ProfilePathValidation?,
		register: @escaping (_ path: String, _ name: String) async -> Bool,
		browseForFolder: @escaping () async -> String? = FolderPicker.choose,
		onCancel: @escaping () -> Void = {},
		clearFocus: @escaping () -> Void,
		validationDelay: Duration = ProfileFormModel.validationDelay
	) {
		self.hasCancel = hasCancel
		self.validate = validate
		self.register = register
		self.browseForFolder = browseForFolder
		self.onCancel = onCancel
		self.clearFocus = clearFocus
		self.validationDelay = validationDelay
		if !initialPath.isEmpty {
			path = initialPath
			validateNow()
		}
	}

	var isNameLocked: Bool {
		if case .existing = status { true } else { false }
	}

	/// The name the field shows: an existing profile's own, otherwise the one typed.
	var name: String {
		if case .existing(let name) = status { name } else { typedName }
	}

	var submitLabel: String { isNameLocked ? "Register Profile" : "Create Profile" }

	/// A validated path, and a name unless the profile exists, with nothing in flight.
	var canSubmit: Bool {
		guard !validating, !submitting else { return false }
		switch status {
		case .existing: return true
		case .new: return !typedName.trimmingCharacters(in: .whitespaces).isEmpty
		case .invalid, nil: return false
		}
	}

	/// An edit of the path: it is validated once no other edit has come for `validationDelay`.
	func editPath(_ path: String) {
		guard path != self.path else { return }
		self.path = path
		validation?.cancel()
		validating = true
		let delay = validationDelay
		validation = Task { [weak self] in
			try? await Task.sleep(for: delay)
			guard !Task.isCancelled else { return }
			await self?.runValidation()
		}
	}

	/// Picks a folder with the system's panel, and validates it at once.
	func browse() {
		Task {
			guard let folder = await browseForFolder() else { return }
			path = folder
			validateNow()
		}
	}

	func submit() {
		guard canSubmit else { return }
		submitting = true
		let (path, name) = (path, typedName.trimmingCharacters(in: .whitespaces))
		Task {
			_ = await register(path, name)
			submitting = false
		}
	}

	func cancel() { onCancel() }

	/// Moves the focus to the next control, or to the previous one when `backward`; from none, to the first or the last.
	/// A locked name and a disabled submit button are skipped, as a browser skips disabled controls.
	func cycleFocus(backward: Bool) {
		let order = Focus.allCases.filter { control in
			switch control {
			case .name: !isNameLocked
			case .submit: canSubmit
			case .cancel: hasCancel
			case .path, .browse: true
			}
		}
		guard let current = focus, let index = order.firstIndex(of: current) else {
			focus = backward ? order.last : order.first
			return
		}
		focus = order[(index + (backward ? order.count - 1 : 1)) % order.count]
	}

	/// Enter submits from a field or the submit button, and presses the other buttons; Space presses the focused button.
	/// Tab and Escape are the container's; typing goes to the focused field, and nothing beneath the form may see a plain key.
	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.returnKey, KeyCode.keypadEnter:
			guard let focus else { return true }
			press(focus == .name || focus == .path ? .submit : focus)
		case KeyCode.space where focus.map { !$0.isTextField } ?? false:
			press(focus!)
		default:
			return focus?.isTextField != true && key.isUnusedOnAPage
		}
		return true
	}

	private func press(_ control: Focus) {
		switch control {
		case .browse: browse()
		case .cancel: cancel()
		case .submit, .name, .path: submit()
		}
	}

	/// Ember validates a path set at once, rather than after a pause: the initial one, and a browsed one.
	private func validateNow() {
		validation?.cancel()
		validating = true
		validation = Task { [weak self] in await self?.runValidation() }
	}

	private func runValidation() async {
		let path = path
		guard !path.trimmingCharacters(in: .whitespaces).isEmpty else {
			// An untouched or cleared field isn't scolded; submitting just stays disabled.
			status = nil
			validating = false
			return
		}
		let result = await validate(path)
		guard !Task.isCancelled else { return }
		status = result.map(Status.init)
		validating = false
	}
}

/// The system's folder panel, as Ember's "Browse…" opens the dialog plugin's.
enum FolderPicker {
	@MainActor
	static func choose() async -> String? {
		let panel = NSOpenPanel()
		panel.canChooseDirectories = true
		panel.canChooseFiles = false
		panel.canCreateDirectories = true
		panel.allowsMultipleSelection = false
		panel.prompt = "Choose"
		let response = if let window = NSApp.keyWindow {
			await panel.beginSheetModal(for: window)
		} else {
			panel.runModal()
		}
		return response == .OK ? panel.url?.path : nil
	}
}

/// The form's fields, its validation status, what a profile root holds, and its buttons.
struct ProfileFormView: View {
	@Bindable var model: ProfileFormModel
	@FocusState private var field: ProfileFormModel.Focus?

	var body: some View {
		VStack(alignment: .leading, spacing: 14) {
			VStack(alignment: .leading, spacing: 6) {
				label("Name")
				TextField("e.g. Personal", text: Binding(get: { model.name }, set: { model.typedName = $0 }))
					.textFieldStyle(.roundedBorder)
					.disabled(model.isNameLocked)
					.focused($field, equals: .name)
					.accessibilityIdentifier("profile-form-name")
			}
			VStack(alignment: .leading, spacing: 6) {
				label("Root Path")
				HStack(spacing: 8) {
					TextField("~/devora", text: Binding(get: { model.path }, set: { model.editPath($0) }))
						.textFieldStyle(.roundedBorder)
						.focused($field, equals: .path)
						.accessibilityIdentifier("profile-form-path")
					DialogButton(title: "Browse…", isFocused: model.focus == .browse, action: model.browse)
						.accessibilityIdentifier("profile-form-browse")
				}
				if let status = model.status {
					StatusLine(status: status)
				}
			}
			ProfileRootInfo()
			HStack(spacing: 10) {
				DialogButton(title: model.submitLabel, isFocused: model.focus == .submit, tint: .accentColor, action: model.submit)
					.disabled(!model.canSubmit)
					.accessibilityIdentifier("profile-form-submit")
				if model.hasCancel {
					DialogButton(title: "Cancel", isFocused: model.focus == .cancel, action: model.cancel)
				}
			}
		}
		.onChange(of: model.focus, initial: true) { _, focus in field = focus?.isTextField == true ? focus : nil }
		.onChange(of: field) { _, field in
			if let field { model.focus = field }
		}
	}

	private func label(_ text: String) -> some View {
		Text(text).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
	}
}

private struct StatusLine: View {
	let status: ProfileFormModel.Status

	var body: some View {
		Group {
			switch status {
			case .new:
				Text("✓ New profile — the structure below will be created").foregroundStyle(.green)
			case .existing(let name):
				Text("◆ Existing profile \"\(name)\" found — it will be registered as-is").foregroundStyle(.blue)
			case .invalid(let error):
				Text("✗ \(error)").foregroundStyle(.red)
			}
		}
		.font(.callout)
		.accessibilityIdentifier("profile-form-status")
	}
}

/// What a profile root holds, as Ember's form lists it.
private struct ProfileRootInfo: View {
	var body: some View {
		VStack(alignment: .leading, spacing: 4) {
			Text("Inside the profile root:").font(.callout.weight(.semibold))
			ForEach(
				[
					("config.json", "profile-specific configuration"),
					("repos/", "clone your git repos here (worktrees are created from them)"),
					("workspaces/", "Devora-managed worktrees, one directory per workspace"),
				], id: \.0
			) { entry, description in
				Text("\(Text(entry).font(.callout.monospaced())) — \(description)")
			}
		}
		.font(.callout)
		.foregroundStyle(.secondary)
		.padding(10)
		.frame(maxWidth: .infinity, alignment: .leading)
		.background(.quaternary.opacity(0.4), in: .rect(cornerRadius: 8))
	}
}

extension ProfileFormModel {
	/// A form over the store's calls; `onRegistered` gets the registered profile's path.
	convenience init(
		store: Store, initialPath: String = "", hasCancel: Bool, onCancel: @escaping () -> Void = {},
		clearFocus: @escaping () -> Void, onRegistered: @escaping (String) -> Void
	) {
		self.init(
			initialPath: initialPath, hasCancel: hasCancel,
			validate: { await store.validateProfilePath($0) },
			register: { path, name in
				guard let profile = await store.registerProfile(path: path, name: name) else { return false }
				onRegistered(profile.path)
				return true
			},
			onCancel: onCancel, clearFocus: clearFocus)
	}
}
