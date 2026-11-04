#!/bin/sh
# Requires existing, user-configured SDK/tools. No setup, signing or device install.
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
task_sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
task_gradle=${1:-gradle}
if [ -z "$task_sdk" ] || [ ! -f "$task_sdk/platforms/android-37.2/android.jar" ] || [ ! -d "$task_sdk/build-tools/37.0.0" ]; then
    echo "Existing Android SDK platform 37.2 and build tools 37.0.0 required. Configure ANDROID_HOME." >&2
    exit 2
fi
export ANDROID_HOME="$task_sdk"
export GRADLE_USER_HOME="$task_root/target/gradle-home"
task_version=$("$task_gradle" --version)
if ! printf '%s\n' "$task_version" | grep -q '^Gradle 9\.8\.2[[:space:]]*$'; then
    echo "A verified Gradle 9.8.2 installation is required." >&2
    exit 2
fi
"$task_gradle" -p "$task_root/apps/android" --no-daemon --console=plain :app:assembleDebug :app:lintDebug
