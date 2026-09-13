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

    // One JVM target for Java and Kotlin; Gradle rejects a mismatch between them.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets.named("main") {
        java.srcDir(rootProject.file("../../bindings/generated/android/kotlin"))
        jniLibs.srcDir(rootProject.file("../../bindings/generated/android/jniLibs"))
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.1")
    // Supplies Dispatchers.Main for the Android observer.
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.1")
}
