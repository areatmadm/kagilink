#!/bin/sh
# Builds the Rust C ABI (areatm-kagilink-ffi) for Apple targets and packages Binary/KagiLinkC.xcframework.
# Local build only: no signing, account, installation or publication. Synthetic-data spike (D021).
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
root=$(CDPATH= cd -- "$here/../../.." && pwd)
command -v cargo >/dev/null 2>&1 || { echo "cargo (rustup toolchain 1.92.0) is required." >&2; exit 2; }
command -v xcodebuild >/dev/null 2>&1 || { echo "Full Xcode is required." >&2; exit 2; }
targets="aarch64-apple-darwin aarch64-apple-ios aarch64-apple-ios-sim"
for t in $targets; do
    rustup target list --installed 2>/dev/null | grep -qx "$t" || { echo "Missing rust target $t. Nothing was built." >&2; exit 2; }
done
# Deployment floors: iOS 18, macOS 14. panic=abort per docs/FFI_DESIGN.md for release packaging.
export IPHONEOS_DEPLOYMENT_TARGET=18.0 MACOSX_DEPLOYMENT_TARGET=14.0
export CARGO_PROFILE_RELEASE_PANIC=abort
args=""
for t in $targets; do
    (cd "$root" && cargo build -p areatm-kagilink-ffi --release --locked --target "$t")
    args="$args -library $root/target/$t/release/libareatm_kagilink_ffi.a -headers $here/.headers"
done
rm -rf "$here/.headers" "$here/Binary"
mkdir -p "$here/.headers" "$here/Binary"
cp "$root/core/kagilink-ffi/include/kagilink.h" "$here/.headers/kagilink.h"
cat > "$here/.headers/module.modulemap" <<'M'
module KagiLinkC {
    header "kagilink.h"
    export *
}
M
# shellcheck disable=SC2086
xcodebuild -create-xcframework $args -output "$here/Binary/KagiLinkC.xcframework"
rm -rf "$here/.headers"
echo "Built $here/Binary/KagiLinkC.xcframework"
