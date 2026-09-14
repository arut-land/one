plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "dev.arut.surface"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.arut.app"
        minSdk = 24
        targetSdk = 37
        versionCode = 1
        versionName = "0.1.0"
    }

    // One JVM target for Java and Kotlin; Gradle rejects a mismatch between them.
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures {
        compose = true
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}

dependencies {
    // The generated FFI package, the JNI libraries, and the observation helpers
    // this app's screens read Rust through (ADR 0007).
    implementation(project(":bindings"))
    implementation(libs.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.foundation)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.material3)
    // currentWindowAdaptiveInfo(): the window's own breakpoints, not a dp
    // measurement of ours; material3-window-size-class is deprecated in favour of it.
    implementation(libs.compose.material3.adaptive)
    // The WindowSizeClass type ChatScreen's signature names.
    implementation(libs.window.core)
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)
    // ADR 0011: the session lives in a ViewModel so it survives configuration changes.
    implementation(libs.lifecycle.viewmodel.compose)
    // collectAsStateWithLifecycle: stop collecting Rust revisions in the background.
    implementation(libs.lifecycle.runtime.compose)
    // Supplies Dispatchers.Main for the generated callbackFlow streams.
    implementation(libs.coroutines.android)
}
