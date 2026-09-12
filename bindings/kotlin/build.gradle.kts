plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.arut.bindings"
    compileSdk = 35

    defaultConfig {
        minSdk = 24
    }

    sourceSets.named("main") {
        java.srcDir(rootProject.file("../../bindings/generated/android/kotlin"))
        jniLibs.srcDir(rootProject.file("../../bindings/generated/android/jniLibs"))
    }
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.1")
    // Provides the Dispatchers.Main implementation on Android; ObservableState
    // falls back to Dispatchers.Default when this module is absent (e.g. a
    // plain JVM consumer).
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")
}
