import Foundation
import WebKit

/// Serves only files inside the bundled renderer (or `root`, in tests), with one local origin for module imports.
public final class RendererSchemeHandler: NSObject, WKURLSchemeHandler {
	private let root: URL?

	public init(root: URL? = Bundle.main.resourceURL?.appending(path: "web-renderer", directoryHint: .isDirectory)) {
		self.root = root?.resolvingSymlinksInPath().standardizedFileURL
	}

	public func webView(_ webView: WKWebView, start task: any WKURLSchemeTask) {
		guard let url = task.request.url, url.host == "app", let root else {
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
	public func webView(_ webView: WKWebView, stop task: any WKURLSchemeTask) {}
}
