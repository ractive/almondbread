#!/usr/bin/env bash
# Builds the Rust core (crates/almondbread-ffi) for macOS arm64, iOS arm64 and the iOS
# simulator (arm64), generates the Swift bindings with uniffi-bindgen (library mode) and
# packages build/rust/AlmondbreadCoreFFI.xcframework (static libs + C header + module map).
# The generated Swift file goes to Sources/AlmondbreadCore/Generated/. kb: platform/rust-swift-ffi
#
#   scripts/build-rust-xcframework.sh            # release (default)
#   PROFILE=debug scripts/build-rust-xcframework.sh
#
# Incremental: cargo decides what to rebuild; the XCFramework and the bindings are only
# replaced when their content changes, so Xcode and SwiftPM do not relink needlessly.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export DEVELOPER_DIR="${DEVELOPER_DIR:-/Applications/Xcode.app/Contents/Developer}"
# Match the Swift package's deployment targets (Package.swift: macOS 14, iOS 17).
export MACOSX_DEPLOYMENT_TARGET=14.0
export IPHONEOS_DEPLOYMENT_TARGET=17.0

PROFILE="${PROFILE:-release}"
CARGO_FLAGS=()
[ "$PROFILE" = release ] && CARGO_FLAGS+=(--release)

TARGETS=(aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim)
LIB=libalmondbread_ffi.a
OUT="$ROOT/build/rust"
XCF_NAME=AlmondbreadCoreFFI
XCF="$OUT/$XCF_NAME.xcframework"
GEN_SWIFT_DIR="$ROOT/Sources/AlmondbreadCore/Generated"

installed="$(rustup target list --installed)"
for t in "${TARGETS[@]}"; do
    grep -qx "$t" <<<"$installed" || rustup target add "$t"
done

for t in "${TARGETS[@]}"; do
    cargo build "${CARGO_FLAGS[@]}" -p almondbread-ffi --lib --target "$t"
done

# Bindings from the host library's embedded UniFFI metadata.
mkdir -p "$OUT"
BIND="$OUT/bindings.tmp"
rm -rf "$BIND"
cargo run --quiet --release -p uniffi-bindgen -- generate \
    --library "target/aarch64-apple-darwin/$PROFILE/libalmondbread_ffi.dylib" \
    --language swift --out-dir "$BIND" >/dev/null

HEADERS="$OUT/headers.tmp"
rm -rf "$HEADERS"
mkdir -p "$HEADERS"
cp "$BIND/almondbread_ffiFFI.h" "$HEADERS/"
cp "$BIND/almondbread_ffiFFI.modulemap" "$HEADERS/module.modulemap"

# Rebuild the XCFramework only when a library or the header changed.
stamp="$OUT/.xcframework.sha256"
sum="$( (for t in "${TARGETS[@]}"; do shasum -a 256 "target/$t/$PROFILE/$LIB"; done
         shasum -a 256 "$HEADERS"/*) | shasum -a 256 | cut -d' ' -f1)"
if [ ! -d "$XCF" ] || [ ! -f "$stamp" ] || [ "$(cat "$stamp")" != "$sum" ]; then
    tmp="$OUT/$XCF_NAME.tmp.xcframework"
    rm -rf "$tmp"
    args=()
    for t in "${TARGETS[@]}"; do
        args+=(-library "target/$t/$PROFILE/$LIB" -headers "$HEADERS")
    done
    xcodebuild -create-xcframework "${args[@]}" -output "$tmp" >/dev/null
    # Swap in place so a concurrent build never sees a half-written framework.
    rm -rf "$XCF.old"
    [ -d "$XCF" ] && mv "$XCF" "$XCF.old"
    mv "$tmp" "$XCF"
    rm -rf "$XCF.old"
    echo "$sum" >"$stamp"
    echo "rust: built $XCF"
else
    echo "rust: $XCF up to date"
fi

mkdir -p "$GEN_SWIFT_DIR"
if ! cmp -s "$BIND/almondbread_ffi.swift" "$GEN_SWIFT_DIR/almondbread_ffi.swift"; then
    cp "$BIND/almondbread_ffi.swift" "$GEN_SWIFT_DIR/almondbread_ffi.swift"
    echo "rust: updated $GEN_SWIFT_DIR/almondbread_ffi.swift"
fi
rm -rf "$BIND" "$HEADERS"
