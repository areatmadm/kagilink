plugins {
    id("com.android.application")
}
android {
    namespace = "in.minori.key.phase0.strongbox"
    compileSdk {
        version = release(37) { minorApiLevel = 2 }
    }
    buildToolsVersion = "37.0.0"
    defaultConfig {
        applicationId = "in.minori.key.phase0.strongbox"
        minSdk = 34
        targetSdk = 37
        versionCode = 1
        versionName = "0.0.0-phase0-strongbox"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildTypes {
        debug { signingConfig = null }
        release { isMinifyEnabled = false; signingConfig = null }
    }
    // A probe must not accidentally become a signed product release.
}
kotlin {
    compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
}
dependencyLocking { lockAllConfigurations() }
