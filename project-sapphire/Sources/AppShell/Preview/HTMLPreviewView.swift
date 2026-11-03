import SwiftUI
import WebKit

/// An untrusted HTML document, isolated from the Markdown renderer and app resources.
struct HTMLPreviewView: NSViewRepresentable {
	let content: () throws -> String
	let onError: (any Error) -> Void
	let onView: (WKWebView) -> Void
	let revision: String

	func makeCoordinator() -> Coordinator { Coordinator(content: content, onError: onError, revision: revision) }

	static func configuration() -> WKWebViewConfiguration {
		let configuration = WKWebViewConfiguration()
		configuration.websiteDataStore = .nonPersistent()
		configuration.defaultWebpagePreferences.allowsContentJavaScript = false
		configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
		// No custom schemes, user scripts or message handlers belong in this view.
		return configuration
	}

	func makeNSView(context: Context) -> WKWebView {
		let view = WKWebView(frame: .zero, configuration: Self.configuration())
		view.navigationDelegate = context.coordinator
		view.uiDelegate = context.coordinator
		onView(view)
		context.coordinator.prepare(view)
		return view
	}

	func updateNSView(_ view: WKWebView, context: Context) {
		context.coordinator.content = content
		context.coordinator.revision = revision
		context.coordinator.render(view)
	}

	static func dismantleNSView(_ view: WKWebView, coordinator: Coordinator) {
		coordinator.stop()
		view.stopLoading()
		view.navigationDelegate = nil
		view.uiDelegate = nil
	}

	@MainActor
	final class Coordinator: NSObject, WKNavigationDelegate, WKUIDelegate {
		typealias Compile = @MainActor (@escaping @MainActor (WKContentRuleList?, (any Error)?) -> Void) -> Void
		var content: () throws -> String
		var revision: String
		private let onError: (any Error) -> Void
		private let compile: Compile
		private var ready = false
		private var stopped = false
		private var loadingNavigation: WKNavigation?
		private var loadingRevision: String?
		private var renderedRevision: String?
		private var initialNavigationPending = false

		init(content: @escaping () throws -> String, onError: @escaping (any Error) -> Void, revision: String, compile: @escaping Compile = compileRules) {
			self.content = content
			self.onError = onError
			self.revision = revision
			self.compile = compile
		}

		/// Even the first document waits for the blocker; a failed setup never loads content.
		func prepare(_ view: WKWebView) {
			compile { [weak self, weak view] rules, error in
				guard let self, let view, !self.stopped else { return }
				guard let rules, error == nil else {
					self.onError(error ?? CocoaError(.fileReadUnknown))
					return
				}
				view.configuration.userContentController.add(rules)
				self.ready = true
				self.render(view)
			}
		}

		func stop() { stopped = true }

		func render(_ view: WKWebView) {
			guard ready, !stopped, loadingRevision == nil, revision != renderedRevision else { return }
			do {
				let html = try content()
				loadingRevision = revision
				initialNavigationPending = true
				// Nil base URL grants no directory access. The first policy precedes any author markup.
				loadingNavigation = view.loadHTMLString(Self.protectedDocument(html), baseURL: nil)
			} catch {
				renderedRevision = revision
				onError(error)
			}
		}

		private static func compileRules(_ completion: @escaping @MainActor (WKContentRuleList?, (any Error)?) -> Void) {
			WKContentRuleListStore.default().compileContentRuleList(forIdentifier: "devora-html-no-resources-v1", encodedContentRuleList: """
				[{"trigger":{"url-filter":".*"},"action":{"type":"block"}}]
				""", completionHandler: completion)
		}

		private static func protectedDocument(_ html: String) -> String {
			"""
			<!doctype html><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src 'none'; font-src 'none'; media-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">
			\(html)
			"""
		}

		func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction, decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void) {
			// Only our inline load may navigate. This also refuses frames, refreshes, forms,
			// downloads, external-app URLs and target=_blank; nothing opens in the browser.
			let initial = !stopped && initialNavigationPending && action.targetFrame?.isMainFrame == true
				&& action.navigationType == .other && action.request.url?.absoluteString == "about:blank"
			if initial { initialNavigationPending = false }
			decisionHandler(initial ? .allow : .cancel)
		}

		func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration, for action: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? { nil }

		func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
			guard !stopped, navigation === loadingNavigation else { return }
			loadingNavigation = nil
			renderedRevision = loadingRevision
			loadingRevision = nil
			initialNavigationPending = false
			render(webView)
		}

		func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: any Error) { failed(webView, navigation, error) }
		func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: any Error) { failed(webView, navigation, error) }

		private func failed(_ view: WKWebView, _ navigation: WKNavigation?, _ error: any Error) {
			guard !stopped, navigation === loadingNavigation, let loading = loadingRevision else { return }
			loadingNavigation = nil
			renderedRevision = loading
			loadingRevision = nil
			initialNavigationPending = false
			if revision == loading { onError(error) } else { render(view) }
		}
	}
}
