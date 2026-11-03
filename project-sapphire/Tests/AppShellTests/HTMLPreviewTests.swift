import Foundation
import Testing
import WebKit
@testable import AppShell

@MainActor
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

	private func heading(_ expected: String, in view: WKWebView) async throws {
		let deadline = Date.now.addingTimeInterval(10)
		while Date.now < deadline {
			if let value = try? await view.evaluateJavaScript("document.querySelector('h1')?.textContent"), value as? String == expected { return }
			try await Task.sleep(for: .milliseconds(50))
		}
		Issue.record("HTML heading did not become \(expected)")
	}
}
