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
	@ObservationIgnored private var pendingG = false

	init(store: Store) { self.store = store }

	func showError(_ error: any Error) { self.error = String(describing: error) }

	func content() throws -> (String, [String: String]) {
		guard let resourceDir = Bundle.main.resourcePath else {
			throw CocoaError(.fileNoSuchFile)
		}
		return try store.userGuideContent(resourceDir: resourceDir)
	}

	func handleKey(_ key: Layers.KeyPress) -> Bool {
		let completingG = pendingG
		pendingG = false
		var action: String?
		if key.modifiers.isEmpty, key.characters == "g" {
			if completingG { action = "top" } else { pendingG = true }
			if action == nil { return true }
		} else if key.modifiers.isDisjoint(with: [.control, .option, .command]), key.characters == "G" {
			action = "bottom"
		} else if key.modifiers.isEmpty, key.characters == "j" || key.keyCode == KeyCode.downArrow {
			action = "down"
		} else if key.modifiers.isEmpty, key.characters == "k" || key.keyCode == KeyCode.upArrow {
			action = "up"
		} else if key.modifiers == .control, key.keyCode == KeyCode.d {
			action = "halfDown"
		} else if key.modifiers == .control, key.keyCode == KeyCode.u {
			action = "halfUp"
		}
		guard let action else { return key.isUnusedOnAPage }
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
				GuideWebView(model: model)
			}
			Divider()
			KeyHintBar(hints: [("j/k · Ctrl+D/U · gg/G", "scroll"), ("q/Esc", "close")])
		}
		.background(.windowBackground)
		.accessibilityIdentifier(UserGuideModel.layerName)
	}
}

private struct GuideWebView: NSViewRepresentable {
	let model: UserGuideModel

	func makeCoordinator() -> Coordinator { Coordinator(model: model) }

	func makeNSView(context: Context) -> WKWebView {
		let configuration = WKWebViewConfiguration()
		configuration.setURLSchemeHandler(RendererSchemeHandler(), forURLScheme: "devora-renderer")
		let view = WKWebView(frame: .zero, configuration: configuration)
		view.navigationDelegate = context.coordinator
		model.webView = view
		view.load(URLRequest(url: URL(string: "devora-renderer://app/index.html")!))
		return view
	}

	func updateNSView(_ view: WKWebView, context: Context) {}

	@MainActor
	final class Coordinator: NSObject, WKNavigationDelegate {
		let model: UserGuideModel
		init(model: UserGuideModel) { self.model = model }

		func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
			do {
				let (markdown, theme) = try model.content()
				webView.callAsyncJavaScript(#"for (let i = 0; i < 50 && !window.devoraRender && !window.devoraLoadError; i++) await new Promise(resolve => setTimeout(resolve, 100)); if (!window.devoraRender) throw new Error(window.devoraLoadError || "Renderer failed to load"); await window.devoraRender(markdown, theme)"#, arguments: ["markdown": markdown, "theme": theme], in: nil, in: .page) { [weak model] result in
					if case .failure(let error) = result { model?.showError(error) }
				}
			} catch { model.showError(error) }
		}

		func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
			model.showError(error)
		}

		func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void) {
			guard let url = navigationAction.request.url else { decisionHandler(.cancel); return }
			let localPage = url.scheme == "devora-renderer" && url.host == "app" && url.path == "/index.html"
			if navigationAction.navigationType == .linkActivated, !localPage {
				if ["http", "https"].contains(url.scheme?.lowercased() ?? "") { NSWorkspace.shared.open(url) }
				decisionHandler(.cancel)
			} else {
				decisionHandler(localPage ? .allow : .cancel)
			}
		}
	}
}

/// Serves only files inside the bundled renderer, with one local origin for module imports.
private final class RendererSchemeHandler: NSObject, WKURLSchemeHandler {
	func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
		guard let url = task.request.url, url.host == "app",
			let root = Bundle.main.resourceURL?.appending(path: "web-renderer", directoryHint: .isDirectory).resolvingSymlinksInPath().standardizedFileURL else {
			task.didFailWithError(CocoaError(.fileNoSuchFile)); return
		}
		let path = root.appending(path: String(url.path.dropFirst())).resolvingSymlinksInPath().standardizedFileURL
		guard path.path.hasPrefix(root.path + "/"), let data = try? Data(contentsOf: path) else {
			task.didFailWithError(CocoaError(.fileNoSuchFile)); return
		}
		let mime: String
		switch path.pathExtension.lowercased() {
		case "html": mime = "text/html"
		case "js": mime = "text/javascript"
		case "css": mime = "text/css"
		case "svg": mime = "image/svg+xml"
		case "woff": mime = "font/woff"
		case "woff2": mime = "font/woff2"
		default: mime = "application/octet-stream"
		}
		let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": mime, "Access-Control-Allow-Origin": "*"])!
		task.didReceive(response)
		task.didReceive(data)
		task.didFinish()
	}
	func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {}
}
