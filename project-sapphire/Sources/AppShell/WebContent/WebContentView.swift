import AppKit
import SwiftUI
import WebKit

/// A local, navigation-restricted Markdown web pane, shared by the guide and future previews.
struct WebContentView: NSViewRepresentable {
	let content: () throws -> (String, [String: String])
	let onError: (any Error) -> Void
	let onView: (WKWebView) -> Void

	func makeCoordinator() -> Coordinator { Coordinator(content: content, onError: onError) }

	func makeNSView(context: Context) -> WKWebView {
		let configuration = WKWebViewConfiguration()
		configuration.setURLSchemeHandler(RendererSchemeHandler(), forURLScheme: "devora-renderer")
		configuration.userContentController.add(context.coordinator, name: "devoraRenderer")
		let view = WKWebView(frame: .zero, configuration: configuration)
		view.navigationDelegate = context.coordinator
		onView(view)
		view.load(URLRequest(url: URL(string: "devora-renderer://app/index.html")!))
		return view
	}

	func updateNSView(_ view: WKWebView, context: Context) {}

	@MainActor
	final class Coordinator: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
		let content: () throws -> (String, [String: String])
		let onError: (any Error) -> Void
		private var rendered = false
		init(content: @escaping () throws -> (String, [String: String]), onError: @escaping (any Error) -> Void) {
			self.content = content
			self.onError = onError
		}

		func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
			guard message.frameInfo.isMainFrame, let body = message.body as? [String: Any], let webView = message.webView else { return }
			if let error = body["error"] as? String {
				onError(NSError(domain: "WebContent", code: 1, userInfo: [NSLocalizedDescriptionKey: error]))
				return
			}
			guard body["ready"] as? Bool == true, !rendered else { return }
			rendered = true
			do {
				let (markdown, theme) = try content()
				webView.callAsyncJavaScript("await window.devoraRender(markdown, theme)", arguments: ["markdown": markdown, "theme": theme], in: nil, in: .page) { [onError] result in
					if case .failure(let error) = result { onError(error) }
				}
			} catch { onError(error) }
		}

		func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) {
			onError(error)
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
