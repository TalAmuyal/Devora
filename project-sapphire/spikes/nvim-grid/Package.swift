// swift-tools-version: 6.2
import PackageDescription

let package = Package(
	name: "NvimGridSpike",
	platforms: [.macOS("27.0")],
	products: [
		.library(name: "AppShell", targets: ["AppShell"]),
	],
	targets: [
		// Produced by scripts/build-core.sh; neither is checked in.
		.binaryTarget(name: "NvimCoreFFI", path: "build/NvimCoreFFI.xcframework"),
		.target(name: "CoreBindings", dependencies: ["NvimCoreFFI"]),

		.target(name: "MsgPack"),
		.target(name: "NvimRPC", dependencies: ["MsgPack"]),
		// The pane module: its view entry point is `EditorPaneView`.
		.target(name: "EditorPane", dependencies: ["MsgPack", "NvimRPC", "CoreBindings"]),
		.target(name: "AppShell", dependencies: ["EditorPane", "CoreBindings"]),

		.testTarget(name: "MsgPackTests", dependencies: ["MsgPack"]),
		.testTarget(name: "EditorTests", dependencies: ["EditorPane", "CoreBindings", "MsgPack"]),
	]
)
