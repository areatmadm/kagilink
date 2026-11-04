#!/bin/sh
# Synthetic command fixtures only. This never invokes an Apple compiler or signing tool.
set -eu
task_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
mkdir -p "$task_root/target/apple-script-tests"
task_fixture=$(mktemp -d "$task_root/target/apple-script-tests/case.XXXXXX")
task_bin="$task_fixture/bin"
mkdir -p "$task_bin"
cat > "$task_bin/uname" <<'SCRIPT'
#!/bin/sh
printf '%s\n' Darwin
SCRIPT
cat > "$task_bin/xcrun" <<'SCRIPT'
#!/bin/sh
printf '%s\n' "${MOCK_APPLE_SDK_VERSION-27.1}"
SCRIPT
cat > "$task_bin/xcodebuild" <<'SCRIPT'
#!/bin/sh
printf '%s\n' "$*" >> "$MOCK_APPLE_BUILD_CALLS"
SCRIPT
chmod +x "$task_bin/uname" "$task_bin/xcrun" "$task_bin/xcodebuild"
PATH="$task_bin:$PATH"
export PATH
MOCK_APPLE_BUILD_CALLS="$task_fixture/calls"
export MOCK_APPLE_BUILD_CALLS

sh "$task_root/apps/apple/build-probes.sh"
[ "$(wc -l < "$MOCK_APPLE_BUILD_CALLS" | tr -d ' ')" = 2 ]
grep -q -- '-sdk iphoneos27.1' "$MOCK_APPLE_BUILD_CALLS"
[ "$(grep -c 'CODE_SIGNING_ALLOWED=NO' "$MOCK_APPLE_BUILD_CALLS")" = 2 ]
echo 'PASS: exact SDK 27.1 selects two unsigned builds (mocked)'

MOCK_APPLE_BUILD_CALLS="$task_fixture/simulator-calls"
export MOCK_APPLE_BUILD_CALLS
sh "$task_root/apps/apple/build-probes.sh" --simulator
grep -q -- '-sdk iphonesimulator27.1' "$MOCK_APPLE_BUILD_CALLS"
echo 'PASS: simulator selects exact SDK 27.1 (mocked)'

MOCK_APPLE_BUILD_CALLS="$task_fixture/rejected-calls"
MOCK_APPLE_SDK_VERSION=27.0
export MOCK_APPLE_BUILD_CALLS MOCK_APPLE_SDK_VERSION
task_result=0
sh "$task_root/apps/apple/build-probes.sh" || task_result=$?
[ "$task_result" = 2 ]
[ ! -e "$MOCK_APPLE_BUILD_CALLS" ]
echo 'PASS: wrong SDK refuses before invoking build (mocked)'

task_result=0
sh "$task_root/apps/apple/build-probes.sh" --unexpected || task_result=$?
[ "$task_result" = 2 ]
[ ! -e "$MOCK_APPLE_BUILD_CALLS" ]
echo 'PASS: unknown arguments refuse before invoking build (mocked)'
