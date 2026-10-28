#!/usr/bin/env bash
# Rust crate -> static lib -> UniFFI Swift bindings -> SpikeCoreFFI.xcframework + Sources/CoreBindings/CoreBindings.swift
set -euo pipefail

SPIKE_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CORE_DIR="$SPIKE_DIR/core"
BUILD_DIR="$SPIKE_DIR/build"
TARGET=aarch64-apple-darwin
PROFILE="${SPIKE_PROFILE:-release}"
LIB=sapphire_spike_core

cargo build --manifest-path "$CORE_DIR/Cargo.toml" --target "$TARGET" --profile "$PROFILE" --lib
LIB_DIR="$CORE_DIR/target/$TARGET/$PROFILE"

GEN_DIR="$BUILD_DIR/generated"
rm -rf "$GEN_DIR"
cargo run --manifest-path "$CORE_DIR/Cargo.toml" --quiet --bin uniffi-bindgen -- \
	generate --library "$LIB_DIR/lib$LIB.dylib" --language swift --out-dir "$GEN_DIR"

HEADERS_DIR="$BUILD_DIR/headers"
rm -rf "$HEADERS_DIR"
mkdir -p "$HEADERS_DIR"
cp "$GEN_DIR/${LIB}FFI.h" "$HEADERS_DIR/"
cp "$GEN_DIR/${LIB}FFI.modulemap" "$HEADERS_DIR/module.modulemap"

# Recreating an unchanged xcframework makes xcodebuild relink and re-sign everything downstream (~5 s per no-op build).
XCF="$BUILD_DIR/SpikeCoreFFI.xcframework"
XCF_LIB="$XCF/macos-arm64/lib$LIB.a"
if ! cmp -s "$LIB_DIR/lib$LIB.a" "$XCF_LIB" || ! diff -rq "$HEADERS_DIR" "$XCF/macos-arm64/Headers" >/dev/null 2>&1; then
	rm -rf "$XCF"
	xcodebuild -create-xcframework -library "$LIB_DIR/lib$LIB.a" -headers "$HEADERS_DIR" -output "$XCF" >/dev/null
fi

# Only rewrite the Swift file when it changed, so a Rust-only change doesn't force a Swift recompile.
SWIFT_OUT="$SPIKE_DIR/Sources/CoreBindings/CoreBindings.swift"
mkdir -p "$(dirname "$SWIFT_OUT")"
if ! cmp -s "$GEN_DIR/$LIB.swift" "$SWIFT_OUT"; then
	cp "$GEN_DIR/$LIB.swift" "$SWIFT_OUT"
fi
echo "Built $XCF ($PROFILE)"
