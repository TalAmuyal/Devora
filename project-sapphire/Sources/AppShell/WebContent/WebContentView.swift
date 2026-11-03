import AppKit
import SwiftUI
import WebKit

/// A local, navigation-restricted Markdown web pane, shared by the guide and future previews.
struct WebContentView: NSViewRepresentable {
	let content: () throws -> (String, [String: String])
	let onError: (any Error) -> Void
	let onView: (WKWebView) -> Void
	var revision: String = "guide"

	func makeCoordinator() -> Coordinator { Coordinator(content: content, onError: onError, revision: revision) }

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

	func updateNSView(_ view: WKWebView, context: Context) {
		context.coordinator.content = content
		context.coordinator.revision = revision
		context.coordinator.render(in: view)
	}

	static func dismantleNSView(_ view: WKWebView, coordinator: Coordinator) {
		view.configuration.userContentController.removeScriptMessageHandler(forName: "devoraRenderer")
		view.stopLoading()
	}

	@MainActor
	final class Coordinator: NSObject, WKNavigationDelegate, WKScriptMessageHandler {
		var content: () throws -> (String, [String: String])
		let onError: (any Error) -> Void
		var revision: String
		private var ready = false
		private var rendering = false
		private var renderedRevision: String?
		init(content: @escaping () throws -> (String, [String: String]), onError: @escaping (any Error) -> Void, revision: String) {
			self.revision = revision
			self.content = content
			self.onError = onError
		}

		func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
			guard message.frameInfo.isMainFrame, let body = message.body as? [String: Any], let webView = message.webView else { return }
			if let error = body["error"] as? String {
				onError(NSError(domain: "WebContent", code: 1, userInfo: [NSLocalizedDescriptionKey: error]))
				return
			}
			guard body["ready"] as? Bool == true else { return }
			ready = true
			render(in: webView)
		}

		func render(in webView: WKWebView) {
			guard ready, !rendering, revision != renderedRevision else { return }
			let requested = revision
			do {
				let (markdown, theme) = try content()
				rendering = true
				webView.callAsyncJavaScript("await window.devoraRender(markdown, theme)", arguments: ["markdown": markdown, "theme": theme], in: nil, in: .page) { [weak self, weak webView] result in
					guard let self, let webView else { return }
					self.rendering = false
					self.renderedRevision = requested
					if self.revision == requested, case .failure(let error) = result { self.onError(error) }
					self.render(in: webView)
				}
			} catch {
				renderedRevision = requested
				onError(error)
			}
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
