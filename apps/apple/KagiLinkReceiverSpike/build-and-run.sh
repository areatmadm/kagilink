#!/bin/sh
# Builds the KL-V1 macOS receiver spike (Apple Silicon, macOS 14+) into an app bundle, ad-hoc signed for local use.
# No certificate, account, profile or device identifiers are involved. Synthetic data only.
# Usage: sh build-and-run.sh [--run [--stdout FILE] [-- <app args>]]
#   app args: --autoscan  scan on launch and again after a failed attempt while the code is active
#             --choose-file PATH  a harness types the phone tag into PATH (acts for the user; never auto-connects)
# The app is started through LaunchServices (`open`) so macOS attributes the Bluetooth permission to the app.
set -eu
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
core="$here/../KagiLinkCore"
out=${OUT_DIR:-"$here/../../../target/apple-receiver-spike"}
[ -d "$core/Binary/KagiLinkC.xcframework" ] || { echo "Run KagiLinkCore/build-xcframework.sh first." >&2; exit 2; }
slice="$core/Binary/KagiLinkC.xcframework/macos-arm64"
hdr="$slice/Headers"
rm -rf "$out"; mkdir -p "$out/KLV1ReceiverSpike.app/Contents/MacOS"
app="$out/KLV1ReceiverSpike.app"
xcrun --sdk macosx swiftc -swift-version 5 -parse-as-library -O -target arm64-apple-macos14.0 \
    -module-name KLV1ReceiverSpike -I "$hdr" -Xcc "-fmodule-map-file=$hdr/module.modulemap" \
    "$here"/Sources/*.swift "$core/Sources/KagiLinkCore/Native.swift" "$core/Sources/KagiLinkCore/KagiLink.swift" \
    "$slice/libareatm_kagilink_ffi.a" -o "$app/Contents/MacOS/KLV1ReceiverSpike"
cp "$here/Info.plist" "$app/Contents/Info.plist"
codesign --force --sign - "$app"
echo "Built $app"
if [ "${1-}" = "--run" ]; then
    shift
    stdout=""
    if [ "${1-}" = "--stdout" ]; then stdout=$2; shift 2; fi
    [ "${1-}" = "--" ] && shift
    if [ -n "$stdout" ]; then
        : > "$stdout"
        exec open -n -W --stdout "$stdout" --stderr "$stdout" "$app" --args "$@"
    fi
    exec open -n -W "$app" --args "$@"
fi
