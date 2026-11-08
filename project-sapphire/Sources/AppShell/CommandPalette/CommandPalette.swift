import CoreBindings
import Layers
import SwiftUI

/// One command descriptor is shared by the palette, its actions and the test bridge.
struct PaletteCommand: Identifiable {
	enum Action: Equatable {
		case agentHub, workspaceHub, newShell, repurpose, addRepo, cloneRepo, duplicate, closeSession
		case health, newProfile, settings, switchProfile(String), toggleGlobalPause
	}
	let id: String
	let title: String
	let description: String
	let icon: String
	let action: Action
	var shortcut: AppShortcut?

	static func commands(profiles: [Profile], active: String?, globalPaused: Bool = false) -> [Self] {
		[
			Self(id: "workspace-hub", title: "Workspace Hub", description: "List, filter and open workspaces", icon: "▦", action: .workspaceHub),
			Self(id: "new-shell", title: "New Shell", description: "Open a fresh shell tab", icon: "❯", action: .newShell, shortcut: .newShell),
			Self(id: "repurpose-session", title: "Repurpose Current Session", description: "Replace the finished task with a new one in the same workspace", icon: "↻", action: .repurpose),
			Self(id: "add-repo", title: "Add Repo to Workspace", description: "Add a repo (git worktree) to the current workspace", icon: "⊕", action: .addRepo),
			Self(id: "clone-repo", title: "Clone Repo into Profile", description: "Clone a git repo into the active profile's repos/ directory", icon: "⤓", action: .cloneRepo),
			Self(id: "duplicate-session", title: "Duplicate Current Session", description: "Open the New Task dialog pre-filled to copy the current workspace", icon: "⧉", action: .duplicate),
			Self(id: "close-session", title: "Close Current Session", description: "Close the active session tab", icon: "×", action: .closeSession),
			Self(id: "health-hub", title: "Health Hub", description: "Check Devora dependencies, credentials, and setup", icon: "✚", action: .health),
			Self(id: "global-pause", title: globalPaused ? "Resume Devora" : "Pause Devora", description: "Hold background work at its next graceful boundary", icon: globalPaused ? "▶️" : "⏸️", action: .toggleGlobalPause),
			Self(id: "new-profile", title: "New Profile", description: "Create or register a profile root directory", icon: "＋", action: .newProfile),
			Self(id: "agent-hub", title: "Agent Hub", description: "List, create and open agents", icon: "◎", action: .agentHub, shortcut: .hubs),
			Self(id: "manage-profiles", title: "Settings / Manage Profiles", description: "Open the Settings Hub — switch, create, and delete profiles; edit Claude models, effort, and repos", icon: "⚙", action: .settings),
		] + profiles.filter { $0.path != active }.map {
			Self(id: "switch-profile:\($0.path)", title: "Switch Profile: \($0.name)", description: $0.path, icon: "⇄", action: .switchProfile($0.path))
		}
	}
}

@MainActor
@Observable
final class CommandPaletteModel {
	static let layerName = "command-palette"
	private(set) var commands: [PaletteCommand]
	var filter = "" { didSet { cursor.filterChanged(count: items.count) } }
	var fieldFocused = false
	private(set) var cursor = ListCursor()
	@ObservationIgnored var run: (PaletteCommand.Action) -> Void = { _ in }
	@ObservationIgnored var close: () -> Void = {}
	@ObservationIgnored var isEditingText: () -> Bool = { true }

	init(commands: [PaletteCommand]) { self.commands = commands }
	/// Refresh the dynamic profile rows without losing the user's filter or selection.
	func updateProfiles(_ profiles: [Profile], active: String?, globalPaused: Bool = false) {
		let selectedID = selected?.id
		commands = PaletteCommand.commands(profiles: profiles, active: active, globalPaused: globalPaused)
		if let index = items.firstIndex(where: { $0.id == selectedID }) {
			cursor.select(index)
		} else {
			cursor.filterChanged(count: items.count)
		}
	}
	var items: [PaletteCommand] {
		guard !filter.isEmpty else { return commands }
		return commands.filter { $0.title.localizedCaseInsensitiveContains(filter) || $0.description.localizedCaseInsensitiveContains(filter) }
	}
	var selected: PaletteCommand? { cursor.position(in: items.count).map { items[$0] } }
	func select(_ index: Int) { cursor.select(index) }
	func execute() { if let selected { close(); run(selected.action) } }

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard key.modifiers.isDisjoint(with: [.control, .option, .command]) else { return false }
		switch key.keyCode {
		case KeyCode.escape: close()
		case KeyCode.returnKey, KeyCode.keypadEnter: execute()
		case KeyCode.downArrow: cursor.move(by: 1, in: items.count)
		case KeyCode.upArrow: cursor.move(by: -1, in: items.count)
		default:
			if fieldFocused {
				// SwiftUI mounts the field after the layer is pushed. Keep early typing in the
				// model until the field editor actually owns focus; otherwise q dismisses the page.
				if !isEditingText(), !key.characters.isEmpty,
				   key.characters.rangeOfCharacter(from: .controlCharacters) == nil {
					filter += key.characters
					return true
				}
				return false
			}
			switch key.characters {
			case "j": cursor.move(by: 1, in: items.count)
			case "k": cursor.move(by: -1, in: items.count)
			default: return key.isUnusedOnAPage
			}
		}
		return true
	}
}

struct CommandPaletteView: View {
	@Bindable var model: CommandPaletteModel
	@FocusState private var fieldFocused: Bool

	var body: some View {
		ZStack(alignment: .top) {
			Color.black.opacity(0.38).ignoresSafeArea().onTapGesture { model.close() }
			VStack(spacing: 0) {
				TextField("Search commands…", text: $model.filter)
					.textFieldStyle(.plain).font(.title3).padding(16)
					.focused($fieldFocused).accessibilityIdentifier("command-palette-search")
				Divider()
				ScrollViewReader { proxy in
					ScrollView {
						LazyVStack(spacing: 0) {
						ForEach(Array(model.items.enumerated()), id: \.element.id) { index, command in
							Button { model.select(index); model.execute() } label: {
								HStack(spacing: 12) {
									Text(command.icon).frame(width: 24)
									VStack(alignment: .leading, spacing: 3) {
										Text(command.title)
										Text(command.description).font(.caption).foregroundStyle(.secondary)
									}
									Spacer()
									if let label = command.shortcut?.paletteLabel { Text(label).font(.caption.monospaced()).foregroundStyle(.secondary) }
								}.padding(10).contentShape(Rectangle())
									.background(model.selected?.id == command.id ? Color.accentColor.opacity(0.18) : .clear)
							}.buttonStyle(.plain)
						}
						}
					}.frame(maxHeight: 400)
					.onChange(of: model.selected?.id) { _, id in
						if let id { proxy.scrollTo(id, anchor: .center) }
					}
				}
			}.frame(width: 600).background(.regularMaterial, in: RoundedRectangle(cornerRadius: 12))
				.padding(.top, 70)
		}
		.onChange(of: model.fieldFocused, initial: true) { _, focused in fieldFocused = focused }
		.onChange(of: fieldFocused) { _, focused in model.fieldFocused = focused }
	}
}
