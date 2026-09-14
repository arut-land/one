plugins {
    // One catalog entry per plugin, the same versions the app resolves; an
    // alias carries its version, so no subproject repeats one.
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
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

    // The generated Kotlin and the JNI libraries compile here, once, and the app
    // depends on this module rather than on a directory (ADR 0007).
    sourceSets.named("main") {
        java.srcDir(file("../generated/android/kotlin"))
        jniLibs.srcDir(file("../generated/android/jniLibs"))
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    // Flow and StateFlow are in this module's own API, and the generated streams
    // need Dispatchers.Main; `api` so a consumer sees them without repeating it.
    api(libs.coroutines.android)
}
