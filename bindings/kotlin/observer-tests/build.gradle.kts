plugins { kotlin("jvm") version "2.1.0" }
repositories { mavenCentral() }
kotlin {
    jvmToolchain(17)
    sourceSets.main {
        kotlin.srcDir("../src/main/kotlin")
        kotlin.include("**/ObservableState.kt")
    }
}
dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.10.1")
    testImplementation(kotlin("test-junit"))
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.10.1")
}
