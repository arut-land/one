pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "arut-android"

// The one place the generated Kotlin and the JNI libraries are compiled, and the
// home of the observation helpers every screen reads through (ADR 0007).
include(":bindings")
project(":bindings").projectDir = file("../../bindings/kotlin")
