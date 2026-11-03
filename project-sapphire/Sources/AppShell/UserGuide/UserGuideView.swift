import AppKit
import Layers
import Observation
import Store
import SwiftUI
import WebKit

/// One guide page and its local web view; the window stack owns its lifetime.
@MainActor
@Observable
final class UserGuideModel {
	static let layerName = "user-guide"
	private(set) var error: String?
	@ObservationIgnored weak var webView: WKWebView?
	@ObservationIgnored private let store: Store
	@ObservationIgnored private var keys = VimKeyParser()

	init(store: Store) { self.store = store }

	func showError(_ error: any Error) { self.error = Store.describe(error) }

	func content() throws -> (String, [String: String]) {
		guard let resourceDir = Bundle.main.resourcePath else {
			throw CocoaError(.fileNoSuchFile)
		}
		return try store.userGuideContent(resourceDir: resourceDir)
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		guard let motion = keys.motion(for: key) else { return key.isUnusedOnAPage }
		let action: String
		switch motion {
		case .pending: return true
		case .top: action = "top"
		case .bottom: action = "bottom"
		case .line(let direction): action = direction < 0 ? "up" : "down"
		case .halfPage(let direction): action = direction < 0 ? "halfUp" : "halfDown"
		}
		webView?.callAsyncJavaScript("window.devoraScroll(action)", arguments: ["action": action], in: nil, in: .page) { [weak self] result in
			if case .failure(let error) = result { self?.showError(error) }
		}

		return true
	}
}

struct UserGuideView: View {
	@Bindable var model: UserGuideModel

	var body: some View {
		VStack(spacing: 0) {
			HStack { Text("User Guide").font(.title2.weight(.semibold)); Spacer() }.padding()
			Divider()
			if let error = model.error {
				Text("Failed to load User Guide: \(error)").foregroundStyle(.red).frame(maxWidth: .infinity, maxHeight: .infinity)
			} else {
				WebContentView(content: model.content, onError: model.showError, onView: { model.webView = $0 })
			}
			Divider()
			KeyHintBar(hints: [("j/k · Ctrl+D/U · gg/G", "scroll"), ("q/Esc", "close")])
		}
		.background(.windowBackground)
		.accessibilityIdentifier(UserGuideModel.layerName)
	}
}
