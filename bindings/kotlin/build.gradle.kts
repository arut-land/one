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
}
