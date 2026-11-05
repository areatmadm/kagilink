#!/bin/sh
# Binding spike (D021) build: Rust core -> aarch64-linux-android static lib -> JNI shared lib,
# Kotlin test -> dex. Synthetic data only. Produces target/ffi-spike/ (not committed).
#
# Required environment (no downloads are performed):
#   ANDROID_SDK_ROOT  SDK containing ndk/<NDK_VERSION> and build-tools/<BUILD_TOOLS>
#   KOTLIN_LIB        directory with kotlin-compiler-embeddable, kotlin-stdlib, kotlin-script-runtime,
#                     kotlin-reflect, kotlinx-coroutines-core-jvm and annotations jars (a Gradle lib dir)
# Optional: NDK_VERSION (29.0.14206865), BUILD_TOOLS (37.0.0), API (26), JAVA_HOME
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../../.." && pwd)
: "${ANDROID_SDK_ROOT:?set ANDROID_SDK_ROOT}"
: "${KOTLIN_LIB:?set KOTLIN_LIB}"
NDK_VERSION=${NDK_VERSION:-29.0.14206865}
BUILD_TOOLS=${BUILD_TOOLS:-37.0.0}
API=${API:-26}
NDK="$ANDROID_SDK_ROOT/ndk/$NDK_VERSION"
BIN="$NDK/toolchains/llvm/prebuilt/darwin-x86_64/bin"
[ -d "$BIN" ] || BIN="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin"
CC="$BIN/aarch64-linux-android${API}-clang"
OUT="$ROOT/target/ffi-spike"
TARGET=aarch64-linux-android
mkdir -p "$OUT"

echo "== 1/4 Rust core (release, locked, offline)"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC"
export CC_aarch64_linux_android="$CC" AR_aarch64_linux_android="$BIN/llvm-ar"
( cd "$ROOT" && cargo build -p areatm-kagilink-ffi --release --locked --offline --target $TARGET )
STATIC="$ROOT/target/$TARGET/release/libareatm_kagilink_ffi.a"

echo "== 2/4 JNI shared library"
"$CC" -shared -fPIC -O2 -Wall -Wextra -Werror -fvisibility=hidden \
  -I"$ROOT/core/kagilink-ffi/include" "$HERE/jni/kagilink_jni.c" "$STATIC" \
  -Wl,--exclude-libs,ALL -Wl,--gc-sections -Wl,-z,max-page-size=16384 -Wl,-z,relro,-z,now \
  -o "$OUT/libkagilink_jni.so"

# Also stage the library where apps/android/peripheral (Gradle) picks it up as a jniLib.
mkdir -p "$OUT/jniLibs/arm64-v8a" && cp "$OUT/libkagilink_jni.so" "$OUT/jniLibs/arm64-v8a/"

echo "== 3/4 Kotlin"
JAVA=${JAVA_HOME:+$JAVA_HOME/bin/}java
CP=$(ls "$KOTLIN_LIB"/kotlin-compiler-embeddable-*.jar "$KOTLIN_LIB"/kotlin-stdlib-*.jar \
  "$KOTLIN_LIB"/kotlin-script-runtime-*.jar "$KOTLIN_LIB"/kotlin-reflect-*.jar \
  "$KOTLIN_LIB"/kotlinx-coroutines-core-jvm-*.jar "$KOTLIN_LIB"/annotations-*.jar | tr '\n' ':')
STDLIB=$(ls "$KOTLIN_LIB"/kotlin-stdlib-*.jar | head -1)
rm -rf "$OUT/classes" && mkdir -p "$OUT/classes"
"$JAVA" -cp "$CP" org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -no-stdlib -no-reflect \
  -jvm-target 11 -cp "$STDLIB" -d "$OUT/classes" "$HERE/binding/Native.kt" "$HERE/binding/NativeHandle.kt" "$HERE/binding/Framing.kt" \
  "$HERE/src/FramingReference.kt" "$HERE/src/CodecCrossCheck.kt" "$HERE/src/SpikeMain.kt"

echo "== 4/4 dex"
rm -rf "$OUT/dex" && mkdir -p "$OUT/dex"
"$ANDROID_SDK_ROOT/build-tools/$BUILD_TOOLS/d8" --min-api 34 --output "$OUT/dex" \
  --lib "$ANDROID_SDK_ROOT/platforms/android-36/android.jar" \
  $(find "$OUT/classes" -name '*.class') "$STDLIB"
ls -l "$OUT"
