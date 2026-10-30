#!/usr/bin/env bash
# devora-ffi -> static lib -> UniFFI Swift bindings -> build/DevoraCoreFFI.xcframework + Sources/CoreBindings/CoreBindings.swift
set -euo pipefail

SAPPHIRE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CORE_DIR="$(cd "$SAPPHIRE_DIR/../project-core" && pwd)"
BUILD_DIR="$SAPPHIRE_DIR/build"
TARGET=aarch64-apple-darwin
LIB=devora_ffi

# Cargo runs from the core so its rust-toolchain.toml applies.
(cd "$CORE_DIR" && cargo build --release --target "$TARGET" -p devora-ffi --lib)
LIB_DIR="$CORE_DIR/target/$TARGET/release"

GEN_DIR="$BUILD_DIR/generated"
rm -rf "$GEN_DIR"
# Library mode runs `cargo metadata` in the working directory, so it must run inside the crate.
(cd "$CORE_DIR/crates/devora-ffi" && cargo run --quiet --features bindgen --bin uniffi-bindgen -- \
	generate --library "$LIB_DIR/lib$LIB.dylib" --language swift --out-dir "$GEN_DIR")

HEADERS_DIR="$BUILD_DIR/headers"
rm -rf "$HEADERS_DIR"
# In a subdirectory: SwiftPM copies every xcframework's headers into one include/, where GhosttyKit's module.modulemap would collide with ours.
mkdir -p "$HEADERS_DIR/${LIB}FFI"
cp "$GEN_DIR/${LIB}FFI.h" "$HEADERS_DIR/${LIB}FFI/"
cp "$GEN_DIR/${LIB}FFI.modulemap" "$HEADERS_DIR/${LIB}FFI/module.modulemap"

# Recreating an unchanged xcframework makes xcodebuild relink and re-sign everything downstream.
XCF="$BUILD_DIR/DevoraCoreFFI.xcframework"
XCF_LIB="$XCF/macos-arm64/lib$LIB.a"
if ! cmp -s "$LIB_DIR/lib$LIB.a" "$XCF_LIB" || ! diff -rq "$HEADERS_DIR" "$XCF/macos-arm64/Headers" >/dev/null 2>&1; then
	rm -rf "$XCF"
	xcodebuild -create-xcframework -library "$LIB_DIR/lib$LIB.a" -headers "$HEADERS_DIR" -output "$XCF" >/dev/null
fi

# Only rewrite the Swift file when it changed, so a Rust-only change doesn't force a Swift recompile.
SWIFT_OUT="$SAPPHIRE_DIR/Sources/CoreBindings/CoreBindings.swift"
mkdir -p "$(dirname "$SWIFT_OUT")"
if ! cmp -s "$GEN_DIR/$LIB.swift" "$SWIFT_OUT"; then
	cp "$GEN_DIR/$LIB.swift" "$SWIFT_OUT"
fi
echo "Built $XCF"
