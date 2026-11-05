#!/bin/sh
# Verifies that fault-injection code exists in the debug APK (positive control) and is absent from
# the release APK. Usage: check-no-faults-in-release.sh <debug.apk> <release.apk>
# Compile-time gate: the implementation lives only in src/debug. This is a local check (CI does not
# build the Gradle app); the main source set keeps a no-op test seam (FaultPlan/NoFaults).
set -eu
[ $# -eq 2 ] || { echo "usage: $0 <debug.apk> <release.apk>" >&2; exit 2; }
TOKENS='GARBAGE_M2|TAMPER_M4|TAMPER_SECRET|DROP_AFTER_M3|CHUNK_DUP|CHUNK_DROP|CHUNK_SWAP|NO_RESPONSE|DELAY_M2|FAULT INJECTION|fault injected|TEST ONLY|FaultFactory\$Impl|FaultFactory\$Kind'
dump() { unzip -p "$1" 'classes*.dex' | strings -a; }
if ! dump "$1" | grep -Eq "$TOKENS"; then echo "FAIL: debug APK has no fault code (check is not meaningful)" >&2; exit 1; fi
if dump "$2" | grep -E "$TOKENS"; then echo "FAIL: release APK contains fault-injection code" >&2; exit 1; fi
echo "OK: fault injection present in debug APK, absent from release APK"
