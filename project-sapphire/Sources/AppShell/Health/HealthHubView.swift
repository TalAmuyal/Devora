import CoreBindings
import SwiftUI

struct HealthHubView: View {
	@Bindable var model: HealthHubModel

	var body: some View {
		VStack(spacing: 0) {
			HStack {
				Text("Health Hub").font(.title2.weight(.semibold))
				Spacer()
				if let report = model.report {
					Text(report.version).foregroundStyle(.secondary)
					Text(report.overallLabel).foregroundStyle(color(report.overallState))
				}
				Button("↻ Re-run") { model.rerun() }.accessibilityIdentifier("health-rerun")
			}.padding()
			Divider()
			ScrollView {
				VStack(alignment: .leading, spacing: 18) {
					if model.loading { Text("Running health check…") }
					else if let error = model.error { Text(error).foregroundStyle(.red).accessibilityIdentifier("health-error") }
					else if let report = model.report {
						HStack(alignment: .top) {
							ForEach(report.tiles, id: \.label) { tile in
								VStack(alignment: .leading) {
									Text(tile.label)
									Text("\(tile.met)/\(tile.total)").foregroundStyle(color(tile.state))
									ProgressView(value: Double(tile.met), total: Double(max(tile.total, 1)))
								}.frame(maxWidth: .infinity, alignment: .leading)
							}
						}
						ForEach(report.sections, id: \.title) { section in
							VStack(alignment: .leading, spacing: 8) {
								HStack {
									Text(section.title).font(.headline)
									if let count = section.count { Text(count).foregroundStyle(color(section.state)) }
								}
								ForEach(Array(section.items.enumerated()), id: \.offset) { _, item in
									HStack {
										Circle().fill(color(item.state)).frame(width: 8, height: 8)
										Text(item.name).bold()
										if !item.message.isEmpty { Text(item.message) }
										if let version = item.version { Text(version) }
										if let path = item.path { Text(path).foregroundStyle(.secondary).textSelection(.enabled) }
										Spacer()
										if let hint = item.fixHint {
											Text(hint).font(.system(.body, design: .monospaced))
											Button("Copy") { model.copy(hint) }
										}
									}
								}
							}.padding().frame(maxWidth: .infinity, alignment: .leading)
								.background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
						}
					}
				}.padding().frame(maxWidth: .infinity, alignment: .leading)
			}
			.scrollPosition($model.scroll.position)
			.onScrollGeometryChange(for: VimScroll.Geometry.self) { geometry in
				VimScroll.Geometry(offset: geometry.contentOffset.y + geometry.contentInsets.top,
								   contentHeight: geometry.contentSize.height, viewportHeight: geometry.containerSize.height)
			} action: { _, geometry in model.scroll.geometry = geometry }
			Divider()
			KeyHintBar(hints: [("j/k · Ctrl+D/U · gg/G", "scroll"), ("r", "re-run"), ("q/Esc", "close")])
		}
		.background(.windowBackground)
		.overlay(alignment: .bottom) {
			if let toast = model.toast {
				ToastCapsule(text: toast, identifier: "health-toast")
			}
		}
		.accessibilityIdentifier(HealthHubModel.layerName)
	}

	private func color(_ state: HealthState) -> Color {
		switch state { case .ok: .green; case .warn: .orange; case .error: .red; case .info: .secondary }
	}
}
