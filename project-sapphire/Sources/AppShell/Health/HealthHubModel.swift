import AppKit
import CoreBindings
import Layers
import Observation
import Store
import SwiftUI

/// One page's cancellable check; a generation prevents older results replacing newer ones.
@MainActor
@Observable
final class HealthHubModel {
	static let layerName = "health-hub"
	let store: Store
	var scroll = VimScroll()
	private(set) var report: HealthReport?
	private(set) var error: String?
	private(set) var loading = false
	private(set) var toast: String?
	private(set) var generation = 0
	@ObservationIgnored private var running: HealthRun?
	@ObservationIgnored private var task: Task<Void, Never>?
	@ObservationIgnored private var toastTask: Task<Void, Never>?

	init(store: Store) { self.store = store }

	func rerun() {
		try? running?.cancel()
		task?.cancel()
		generation += 1
		let token = generation
		loading = true
		error = nil
		do {
			let run = try store.newHealthRun()
			running = run
			// The core prefers bundled-apps/debi; plain dev builds retain the PATH fallback.
			let resources = Bundle.main.resourcePath
			let profile = store.selectedProfilePath
			task = Task { [weak self] in
				let result = await Task.detached { Result { try run.run(resourceDir: resources, profilePath: profile) } }.value
				guard let self, self.generation == token else { return }
				self.loading = false
				self.running = nil
				switch result {
				case .success(let report): self.report = report
				case .failure(let error): self.report = nil; self.error = Store.describe(error)
				}
			}
		} catch {
			loading = false
			self.error = Store.describe(error)
		}
	}

	func close() {
		generation += 1
		try? running?.cancel()
		task?.cancel()
		toastTask?.cancel()
	}

	func copy(_ value: String) {
		let pasteboard = NSPasteboard.general
		pasteboard.clearContents()
		toast = pasteboard.setString(value, forType: .string) ? "Copied to clipboard" : "Copy failed"
		toastTask?.cancel()
		toastTask = Task { [weak self] in
			try? await Task.sleep(for: .milliseconds(1500))
			guard !Task.isCancelled else { return }
			self?.toast = nil
		}
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		if key.characters == "r", key.modifiers.isDisjoint(with: [.control, .option, .command]) {
			rerun()
			return true
		}
		return scroll.handle(key) || key.isUnusedOnAPage
	}
}
