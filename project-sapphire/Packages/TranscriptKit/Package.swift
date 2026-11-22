// swift-tools-version: 6.2
import PackageDescription

// The native agent transcript (ADR-014), in a package of its own so that it builds and tests without the core's and
// Ghostty's binaries (SwiftPM checks every binary target of a package, even one a target doesn't use).
let package = Package(
	name: "TranscriptKit",
	platforms: [.macOS("26.6")],
	products: [
		.library(name: "TranscriptKit", targets: ["TranscriptKit"]),
		.library(name: "WebRendering", targets: ["WebRendering"]),
	],
	dependencies: [
		// The transcript's Markdown parser (ADR-014).
		.package(url: "https://github.com/swiftlang/swift-markdown.git", exact: "0.9.0"),
		.package(path: "../TestSupport"),
	],
	targets: [
		// The bundled web renderer's local scheme, shared by the web views and the transcript's diagram drawer.
		.target(name: "WebRendering"),
		// Its view entry point is `TranscriptView`.
		.target(
			name: "TranscriptKit",
			dependencies: ["WebRendering", .product(name: "Markdown", package: "swift-markdown")]
		),
		.testTarget(name: "TranscriptKitTests", dependencies: ["TranscriptKit", "TestSupport"]),
		.testTarget(name: "WebRenderingTests", dependencies: ["WebRendering"]),
	]
)
