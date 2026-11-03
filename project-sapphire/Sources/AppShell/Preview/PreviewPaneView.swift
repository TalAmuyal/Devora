import CoreBindings
import Store
import SwiftUI
import WebKit

/// Transient render errors and the live web view for one durable preview pane.
@MainActor
@Observable
final class PreviewPaneModel {
	var error: String?
	@ObservationIgnored weak var webView: WKWebView?
}

/// The file content is transient; identity, path and refresh revision belong to core state.
struct PreviewPaneView: View {
	let preview: PreviewPane
	let store: Store
	@Bindable var model: PreviewPaneModel
	let close: () -> Void

	var body: some View {
		VStack(spacing: 0) {
			HStack {
				Text(URL(fileURLWithPath: preview.path).lastPathComponent).lineLimit(1)
				Spacer()
				Button(action: close) { Image(systemName: "xmark") }
					.buttonStyle(.plain).accessibilityLabel("Close preview")
			}.padding(10)
			Divider()
			if let error = model.error {
				Text(error).foregroundStyle(.red).textSelection(.enabled)
					.frame(maxWidth: .infinity, maxHeight: .infinity)
			} else if ["html", "htm"].contains(URL(fileURLWithPath: preview.path).pathExtension.lowercased()) {
				HTMLPreviewView(content: { try store.htmlPreviewContent(path: preview.path) },
					onError: { model.error = Store.describe($0) }, onView: { model.webView = $0 },
					revision: "\(preview.id):\(preview.revision)")
			} else {
				WebContentView(content: {
					guard let resources = Bundle.main.resourcePath else { throw CocoaError(.fileNoSuchFile) }
					return try store.previewContent(path: preview.path, resourceDir: resources)
				}, onError: { model.error = Store.describe($0) }, onView: { model.webView = $0 }, revision: "\(preview.id):\(preview.revision)")
			}
		}
		.background(.windowBackground)
		.onChange(of: preview.revision) { model.error = nil }
	}
}
