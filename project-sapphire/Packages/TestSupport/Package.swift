// swift-tools-version: 6.2
import PackageDescription

// What the test targets share: how they wait for asynchronous work (`eventually`).
let package = Package(
	name: "TestSupport",
	platforms: [.macOS("26.6")],
	products: [
		.library(name: "TestSupport", targets: ["TestSupport"]),
	],
	targets: [
		.target(name: "TestSupport"),
	]
)
