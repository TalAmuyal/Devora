import AppKit
import SwiftUI
import WebKit
import WebRendering

/// A local, navigation-restricted Markdown web pane, shared by the guide and the previews, in the system's colors for its
/// appearance (`WebPalette`, on a document's background), which it follows live.
struct WebContentView: NSViewRepresentable {
	let content: () throws -> String
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
		context.coordinator.follow(view)
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
		var content: () throws -> String
		let onError: (any Error) -> Void
		var revision: String
		private var ready = false
		private var rendering = false
		private var renderedRevision: String?
		/// The renderer's colors for the view's appearance.
		private var palette: [String: String] = [:]
		private var follower: PaletteFollower?
		/// Where the reader was before a reload for new colors, restored once the page has rendered again.
		private var pendingScroll: Double?

		init(content: @escaping () throws -> String, onError: @escaping (any Error) -> Void, revision: String) {
			self.revision = revision
			self.content = content
			self.onError = onError
		}

		func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
			guard message.frameInfo.isMainFrame, message.frameInfo.request.url?.scheme == "devora-renderer", message.frameInfo.request.url?.host == "app", let body = message.body as? [String: Any], let webView = message.webView else { return }
			if let error = body["error"] as? String {
				onError(NSError(domain: "WebContent", code: 1, userInfo: [NSLocalizedDescriptionKey: error]))
				return
			}
			guard body["ready"] as? Bool == true else { return }
			ready = true
			render(in: webView)
		}

		/// Takes the view's colors now, and again whenever its appearance or the system's colors change.
		func follow(_ webView: WKWebView) {
			applyPalette(to: webView)
			follower = PaletteFollower(webView) { [weak self, weak webView] in
				guard let self, let webView else { return }
				self.paletteChanged(in: webView)
			}
		}

		/// The page has its colors from its first paint, set before any of its own scripts run, so it never shows a
		/// frame in other colors: nor does what's beyond it (`underPageBackgroundColor`).
		private func applyPalette(to webView: WKWebView) {
			palette = WebPalette.variables(for: webView.effectiveAppearance, page: .textBackgroundColor)
			let json = (try? JSONSerialization.data(withJSONObject: palette)).flatMap { String(data: $0, encoding: .utf8) } ?? "{}"
			let controller = webView.configuration.userContentController
			controller.removeAllUserScripts()
			controller.addUserScript(WKUserScript(
				source: "for (const [name, value] of Object.entries(\(json))) document.documentElement.style.setProperty(name, value);",
				injectionTime: .atDocumentStart, forMainFrameOnly: true))
			var background = NSColor.textBackgroundColor
			webView.effectiveAppearance.performAsCurrentDrawingAppearance { background = NSColor.textBackgroundColor.usingColorSpace(.sRGB) ?? background }
			webView.underPageBackgroundColor = background
		}

		/// Mermaid takes its colors once per page, so new colors are a fresh page, rendered again where the reader was.
		/// The text selection isn't kept.
		private func paletteChanged(in webView: WKWebView) {
			let previous = palette
			applyPalette(to: webView)
			guard palette != previous, ready else { return }
			webView.evaluateJavaScript("document.querySelector('#content')?.scrollTop ?? 0") { [weak self, weak webView] value, _ in
				guard let self, let webView else { return }
				self.pendingScroll = value as? Double
				self.ready = false
				self.renderedRevision = nil
				webView.reload()
			}
		}

		func render(in webView: WKWebView) {
			guard ready, !rendering, revision != renderedRevision else { return }
			let requested = revision
			do {
				let markdown = try content()
				rendering = true
				webView.callAsyncJavaScript("await window.devoraRender(markdown, theme)", arguments: ["markdown": markdown, "theme": palette], in: nil, in: .page) { [weak self, weak webView] result in
					guard let self, let webView else { return }
					self.rendering = false
					self.renderedRevision = requested
					if self.revision == requested, case .failure(let error) = result { self.onError(error) }
					if let scroll = self.pendingScroll {
						self.pendingScroll = nil
						webView.evaluateJavaScript("document.querySelector('#content').scrollTop = \(scroll)")
					}
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
