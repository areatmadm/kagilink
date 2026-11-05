#!/bin/sh
# Runs the spike with app_process from /data/local/tmp (shell uid): NO app is installed, no
# root, no signing change. Everything pushed is deleted afterwards. Synthetic data only.
# Usage: ADB=/path/to/adb run-on-device.sh [serial]
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
OUT="$ROOT/target/ffi-spike"
ADB=${ADB:-adb}
[ $# -ge 1 ] && ADB="$ADB -s $1"
D=/data/local/tmp/kagilink-ffi-spike
trap '$ADB shell "rm -rf $D" >/dev/null 2>&1 || true' EXIT
$ADB shell "mkdir -p $D"
$ADB push -q "$OUT/libkagilink_jni.so" "$OUT/dex/classes.dex" "$D/"
$ADB shell "chmod 755 $D/libkagilink_jni.so"
$ADB shell "CLASSPATH=$D/classes.dex app_process $D com.areatm.kagilink.spike.SpikeMainKt $D/libkagilink_jni.so; echo exit=\$?"
