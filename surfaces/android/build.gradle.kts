plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "dev.arut.surface"
    compileSdk = 35

    defaultConfig {
        applicationId = "dev.arut.app"
        minSdk = 24
        targetSdk = 35
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

    // The generated Kotlin and the JNI libraries compile into this module;
    // there is no wrapper subproject between them and the app (ADR 0007).
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
    implementation(libs.activity.compose)
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.foundation)
    implementation(libs.compose.material.icons)
    implementation(libs.compose.material3)
    implementation(libs.compose.ui.tooling.preview)
    debugImplementation(libs.compose.ui.tooling)
    // ADR 0011: the session lives in a ViewModel so it survives configuration changes.
    implementation(libs.lifecycle.viewmodel.compose)
    // collectAsStateWithLifecycle: stop collecting Rust revisions in the background.
    implementation(libs.lifecycle.runtime.compose)
    // Supplies Dispatchers.Main for the generated callbackFlow streams.
    implementation(libs.coroutines.android)
}
