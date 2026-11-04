buildscript {
    configurations.named("classpath") { resolutionStrategy.activateDependencyLocking() }
}
plugins {
    id("com.android.application") version "9.5.0" apply false
}
