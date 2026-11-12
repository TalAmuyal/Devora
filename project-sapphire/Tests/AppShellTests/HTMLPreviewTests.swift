import Foundation
import Testing
import TestSupport
import WebKit
@testable import AppShell

// The waits are for looks, not times; the limit only catches a hang.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct HTMLPreviewTests {
	@Test func setupFailureNeverReadsOrLoadsTheDocument() {
		var reads = 0
		var errors = 0
		var completion: (@MainActor (WKContentRuleList?, (any Error)?) -> Void)?
		let coordinator = HTMLPreviewView.Coordinator(content: { reads += 1; return "<h1>Unsafe</h1>" },
			onError: { _ in errors += 1 }, revision: "1", compile: { completion = $0 })
		let view = WKWebView(frame: .zero, configuration: HTMLPreviewView.configuration())
		coordinator.prepare(view)
		#expect(reads == 0 && view.url == nil)
		completion?(nil, CocoaError(.fileReadUnknown))
		#expect(errors == 1 && reads == 0 && view.url == nil)
	}

	@Test func dismantlingWhileSetupIsPendingIgnoresItsCompletion() {
		var errors = 0
		var completion: (@MainActor (WKContentRuleList?, (any Error)?) -> Void)?
		let coordinator = HTMLPreviewView.Coordinator(content: { Issue.record("a stopped pane must not read content"); return "" },
			onError: { _ in errors += 1 }, revision: "1", compile: { completion = $0 })
		let view = WKWebView(frame: .zero, configuration: HTMLPreviewView.configuration())
		coordinator.prepare(view)
		coordinator.stop()
		completion?(nil, CocoaError(.fileReadUnknown))
		#expect(errors == 0 && view.url == nil)
	}

	@Test func inlineHTMLRendersWithoutRunningContentJavaScriptAndRefreshes() async throws {
		var html = "<h1>Static HTML</h1><script>document.querySelector('h1').textContent='Script ran'</script>"
		var errors: [String] = []
		let coordinator = HTMLPreviewView.Coordinator(content: { html }, onError: { errors.append(String(describing: $0)) }, revision: "1")
		let view = WKWebView(frame: .zero, configuration: HTMLPreviewView.configuration())
		view.navigationDelegate = coordinator
		view.uiDelegate = coordinator
		defer { coordinator.stop(); view.stopLoading() }
		#expect(!view.configuration.websiteDataStore.isPersistent)
		#expect(!view.configuration.defaultWebpagePreferences.allowsContentJavaScript)
		#expect(view.configuration.urlSchemeHandler(forURLScheme: "devora-renderer") == nil)
		coordinator.prepare(view)
		try await heading("Static HTML", in: view)
		html = "<h1>Refreshed HTML</h1>"
		coordinator.revision = "2"
		coordinator.render(view)
		try await heading("Refreshed HTML", in: view)
		#expect(errors.isEmpty)
	}

	private func heading(_ expected: String, in view: WKWebView, sourceLocation: SourceLocation = #_sourceLocation) async throws {
		try await eventually(looks: 1_000, sourceLocation: sourceLocation) {
			(try? await view.evaluateJavaScript("document.querySelector('h1')?.textContent")) as? String == expected
		}
	}
}
