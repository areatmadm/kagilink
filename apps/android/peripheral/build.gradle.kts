plugins {
    id("com.android.application")
}
android {
    namespace = "com.areatm.kagilink.spike.peripheral"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }
    buildToolsVersion = "37.0.0"
    defaultConfig {
        applicationId = "com.areatm.kagilink.spike.peripheral"
        minSdk = 34
        targetSdk = 37
        versionCode = 1
        versionName = "0.0.0-kl-v1-peripheral-spike"
        ndk { abiFilters += "arm64-v8a" }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    // Unsigned research build only (like :app). No signing config, no release.
    buildTypes {
        debug { signingConfig = null }
        release { isMinifyEnabled = false; signingConfig = null }
    }
    sourceSets {
        // libkagilink_jni.so is produced by apps/android/ffi-spike/build.sh (not by Gradle).
        getByName("main").jniLibs.directories.add("../../../target/ffi-spike/jniLibs")
    }
}
// Share the thin JNI binding (binding/Native.kt only; SpikeMain.kt stays in the test).
android.sourceSets.getByName("main").kotlin.directories.add("../ffi-spike/binding")
// Pure-Kotlin risk acknowledgement logic (tests live in riskack/test and are not part of the app).
android.sourceSets.getByName("main").kotlin.directories.add("../riskack/src")
kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
}
dependencyLocking { lockAllConfigurations() }
