// swift-tools-version: 6.2
import PackageDescription

let package = Package(
	name: "SapphireSpike",
	platforms: [.macOS("27.0")],
	products: [
		.library(name: "AppShell", targets: ["AppShell"]),
	],
	targets: [
		// Produced by scripts/build-core.sh; neither is checked in.
		.binaryTarget(name: "SpikeCoreFFI", path: "build/SpikeCoreFFI.xcframework"),
		.target(name: "CoreBindings", dependencies: ["SpikeCoreFFI"]),

		.target(name: "Store", dependencies: ["CoreBindings"]),
		.target(name: "AppShell", dependencies: ["Store"]),
		.executableTarget(name: "Bench", dependencies: ["Store", "CoreBindings"]),
		.testTarget(name: "StoreTests", dependencies: ["Store", "CoreBindings"]),
	]
)
