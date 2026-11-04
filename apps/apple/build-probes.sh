#!/bin/sh
# Compile only: no certificate creation, account selection, installation or publication.
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
if [ "$(uname -s)" != Darwin ]; then
    echo "Apple probes require macOS and full Xcode; nothing was built." >&2
    exit 2
fi
command -v xcodebuild >/dev/null 2>&1 || { echo "Full Xcode is required." >&2; exit 2; }
task_ios_sdk=iphoneos27.1
case "${1-}" in
    "") ;;
    --simulator) task_ios_sdk=iphonesimulator27.1 ;;
    *) echo "Usage: sh apps/apple/build-probes.sh [--simulator]" >&2; exit 2 ;;
esac
if [ "$#" -gt 1 ]; then
    echo "Usage: sh apps/apple/build-probes.sh [--simulator]" >&2
    exit 2
fi
# SDK selection is explicit. Do not silently build with a generic or older SDK.
# DEVELOPER_DIR may select an existing Xcode per process; never run xcode-select.
task_sdk_version=$(xcrun --sdk "$task_ios_sdk" --show-sdk-version 2>/dev/null) || {
    echo "The selected Xcode must contain the iOS/iPadOS 27.1 SDK ($task_ios_sdk). Nothing was built." >&2
    exit 2
}
if [ "$task_sdk_version" != 27.1 ]; then
    echo "Expected SDK 27.1; found $task_sdk_version. Nothing was built." >&2
    exit 2
fi
for task_target in KagiLink-iOS-Phase0 KagiLink-macOS-Phase0; do
    if [ "$task_target" = KagiLink-iOS-Phase0 ]; then task_sdk=$task_ios_sdk; else task_sdk=macosx; fi
    xcodebuild -project "$task_root/apps/apple/KagiLinkPhase0.xcodeproj" \
        -target "$task_target" -configuration Debug -sdk "$task_sdk" \
        SYMROOT="$task_root/target/apple" OBJROOT="$task_root/target/apple-objects" \
        CODE_SIGNING_ALLOWED=NO build
done
