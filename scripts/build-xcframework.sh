#!/usr/bin/env bash
#
# Build sbql-ffi as a universal XCFramework + generate Swift bindings.
#
# Usage: ./scripts/build-xcframework.sh [--debug]
#
# Output:
#   sbql-macos/Frameworks/SbqlFFI.xcframework
#   sbql-macos/sbql-macos/Generated/sbql_ffi.swift

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PROFILE="release"
PROFILE_DIR="release"
if [[ "${1:-}" == "--debug" ]]; then
    PROFILE="dev"
    PROFILE_DIR="debug"
fi

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
FRAMEWORK_NAME="SbqlFFI"
LIB_NAME="libsbql_ffi.a"
OUTPUT_DIR="$ROOT/sbql-macos/Frameworks"
GENERATED_DIR="$ROOT/sbql-macos/sbql-macos/Generated"
HEADERS_DIR="$ROOT/target/uniffi-headers"

echo "==> Building sbql-ffi for ${TARGETS[*]} (profile: $PROFILE)"

# Step 1: Build for each target
for target in "${TARGETS[@]}"; do
    echo "  -> Building for $target..."
    cargo build \
        --manifest-path "$ROOT/Cargo.toml" \
        -p sbql-ffi \
        --profile "$PROFILE" \
        --target "$target"
done

# Step 2: Create universal binary with lipo
echo "==> Creating universal binary..."
UNIVERSAL_DIR="$ROOT/target/universal-$PROFILE_DIR"
mkdir -p "$UNIVERSAL_DIR"

lipo -create \
    "$ROOT/target/aarch64-apple-darwin/$PROFILE_DIR/$LIB_NAME" \
    "$ROOT/target/x86_64-apple-darwin/$PROFILE_DIR/$LIB_NAME" \
    -output "$UNIVERSAL_DIR/$LIB_NAME"

echo "  -> Universal binary: $UNIVERSAL_DIR/$LIB_NAME"

# Step 3: Generate Swift bindings
echo "==> Generating Swift bindings..."
mkdir -p "$GENERATED_DIR"
mkdir -p "$HEADERS_DIR"

cargo run \
    --manifest-path "$ROOT/Cargo.toml" \
    --bin uniffi-bindgen \
    -p sbql-ffi \
    -- generate \
    --library "$ROOT/target/aarch64-apple-darwin/$PROFILE_DIR/$LIB_NAME" \
    --language swift \
    --out-dir "$HEADERS_DIR"

# Move the .swift file to Generated/ and patch for Swift 6 compatibility
if [ -f "$HEADERS_DIR/sbql_ffi.swift" ]; then
    # Add nonisolated to the continuation callback so it can be used as a C function pointer
    # under SWIFT_DEFAULT_ACTOR_ISOLATION = MainActor
    sed -i '' 's/^fileprivate func uniffiFutureContinuationCallback/nonisolated fileprivate func uniffiFutureContinuationCallback/' "$HEADERS_DIR/sbql_ffi.swift"
    mv "$HEADERS_DIR/sbql_ffi.swift" "$GENERATED_DIR/sbql_ffi.swift"
    echo "  -> Swift bindings: $GENERATED_DIR/sbql_ffi.swift"
fi

# Step 4: Use UniFFI's generated modulemap as the canonical one
# UniFFI generates sbql_ffiFFI.modulemap with the correct module name (sbql_ffiFFI).
# xcodebuild -create-xcframework expects module.modulemap, so rename it.
if [ -f "$HEADERS_DIR/sbql_ffiFFI.modulemap" ]; then
    cp "$HEADERS_DIR/sbql_ffiFFI.modulemap" "$HEADERS_DIR/module.modulemap"
    echo "  -> Module map: $HEADERS_DIR/module.modulemap (from sbql_ffiFFI.modulemap)"
fi

# Step 5: Create XCFramework
echo "==> Creating XCFramework..."
rm -rf "$OUTPUT_DIR/$FRAMEWORK_NAME.xcframework"
mkdir -p "$OUTPUT_DIR"

xcodebuild -create-xcframework \
    -library "$UNIVERSAL_DIR/$LIB_NAME" \
    -headers "$HEADERS_DIR" \
    -output "$OUTPUT_DIR/$FRAMEWORK_NAME.xcframework"

echo "==> Done! XCFramework: $OUTPUT_DIR/$FRAMEWORK_NAME.xcframework"
echo "    Swift bindings:    $GENERATED_DIR/sbql_ffi.swift"
echo ""
echo "Next steps:"
echo "  1. In Xcode, add SbqlFFI.xcframework to your target's Frameworks"
echo "  2. Add Generated/sbql_ffi.swift to your source tree"
echo "  3. Add linker flags: -lsqlite3"
echo "  4. Add frameworks: Security, SystemConfiguration"
