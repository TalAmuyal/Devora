// swift-tools-version: 6.2
import PackageDescription

let package = Package(
	name: "LibghosttySpike",
	platforms: [.macOS("27.0")],
	products: [
		.library(name: "AppShell", targets: ["AppShell"]),
	],
	targets: [
		// Built by `mise run ghostty-build` from the pinned Ghostty commit; not checked in.
		.binaryTarget(name: "GhosttyKit", path: "build/GhosttyKit.xcframework"),
		// The only module that imports GhosttyKit; its view entry point is `TerminalPane`.
		.target(
			name: "TerminalKit",
			dependencies: ["GhosttyKit"],
			linkerSettings: [.linkedLibrary("c++"), .linkedFramework("Carbon")]
		),
		.target(name: "AppShell", dependencies: ["TerminalKit"]),
	]
)
