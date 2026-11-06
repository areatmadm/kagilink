#!/bin/sh
# Compiles and runs the pure-Kotlin risk-ack tests (RiskAckLogic.kt + RiskAckTests.kt).
#   host:   sh run-tests.sh                       (needs KOTLIN_LIB and a java on PATH/JAVA_HOME)
#   device: ADB=<adb> ANDROID_SDK_ROOT=<sdk> sh run-tests.sh device   (dex + app_process under /data/local/tmp, removed after)
# KOTLIN_LIB: directory with kotlin-compiler-embeddable, kotlin-stdlib, kotlin-script-runtime, kotlin-reflect,
# kotlinx-coroutines-core-jvm and annotations jars (a Gradle lib dir). Output goes to target/riskack-tests (untracked).
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
: "${KOTLIN_LIB:?set KOTLIN_LIB}"
OUT="$ROOT/target/riskack-tests"
JAVA=${JAVA_HOME:+$JAVA_HOME/bin/}java
CP=$(ls "$KOTLIN_LIB"/kotlin-compiler-embeddable-*.jar "$KOTLIN_LIB"/kotlin-stdlib-*.jar \
  "$KOTLIN_LIB"/kotlin-script-runtime-*.jar "$KOTLIN_LIB"/kotlin-reflect-*.jar \
  "$KOTLIN_LIB"/kotlinx-coroutines-core-jvm-*.jar "$KOTLIN_LIB"/annotations-*.jar | tr '\n' ':')
STDLIB=$(ls "$KOTLIN_LIB"/kotlin-stdlib-*.jar | head -1)
rm -rf "$OUT" && mkdir -p "$OUT/classes"
"$JAVA" -cp "$CP" org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect -jvm-target 11 \
  -cp "$STDLIB" -d "$OUT/classes" "$HERE/src/RiskAckLogic.kt" "$HERE/test/RiskAckTests.kt" 2>&1 | grep -v '^WARNING' || true
if [ "${1:-host}" = "device" ]; then
  : "${ADB:?set ADB}" "${ANDROID_SDK_ROOT:?set ANDROID_SDK_ROOT}"
  BT=${BUILD_TOOLS:-37.0.0}
  mkdir -p "$OUT/dex"
  "$ANDROID_SDK_ROOT/build-tools/$BT/d8" --min-api 34 --output "$OUT/dex" \
    --lib "$ANDROID_SDK_ROOT/platforms/android-36/android.jar" $(find "$OUT/classes" -name '*.class') "$STDLIB" 2>&1 | grep -v '^WARNING' || true
  D=/data/local/tmp/kagilink-riskack
  trap '"$ADB" shell rm -rf $D' EXIT
  "$ADB" shell rm -rf $D; "$ADB" shell mkdir -p $D
  "$ADB" push "$OUT/dex/classes.dex" $D/classes.dex >/dev/null
  "$ADB" shell "CLASSPATH=$D/classes.dex app_process / com.areatm.kagilink.riskack.RiskAckTestsKt"
else
  "$JAVA" -cp "$OUT/classes:$STDLIB" com.areatm.kagilink.riskack.RiskAckTestsKt
fi
