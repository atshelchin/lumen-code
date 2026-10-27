plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.kotlin.serialization)
}

// src/ — the Rust workspaces live next to this project
val srcRoot: File = rootDir.parentFile

android {
    namespace = "com.shelchin.lumen"
    compileSdk = 36

    defaultConfig {
        applicationId = "ltd.mondaylabs.lumencode"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1.0.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    buildTypes {
        debug {
            buildConfigField("boolean", "SCRIPTS", "true")
        }
        release {
            optimization { enable = false }
            signingConfig = signingConfigs.getByName("debug")
            buildConfigField("boolean", "SCRIPTS", "false")
        }
        // Release code (compiled ahead of time, no JNI checks: the debug build's interpreter
        // makes the UI and every call into Rust several times slower) with the test hooks of
        // DebugScript and simpleperf profiling: what to measure on a device.
        create("bench") {
            initWith(getByName("release"))
            isProfileable = true
            buildConfigField("boolean", "SCRIPTS", "true")
            matchingFallbacks += listOf("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures { compose = true; buildConfig = true }
    sourceSets {
        // uniffi-generated Kotlin (scripts/build-core.sh)
        getByName("main") { kotlin.srcDir(layout.buildDirectory.dir("generated/uniffi").get().asFile) }
    }
    packaging { jniLibs { useLegacyPackaging = false } }
}

java { toolchain { languageVersion = JavaLanguageVersion.of(17) } }

// The Rust core: libraries for each ABI and the Kotlin bindings. Up to date unless something
// under src/app or src/rust changed. Skip with -PlumenSkipRust when only Kotlin changed.
val cargoBuild = tasks.register<Exec>("cargoBuild") {
    group = "build"
    description = "Builds the Rust core (cargo-ndk) and generates the Kotlin bindings"
    workingDir = srcRoot
    commandLine("sh", rootDir.resolve("scripts/build-core.sh").absolutePath)
    enabled = !providers.gradleProperty("lumenSkipRust").isPresent
    inputs.dir(srcRoot.resolve("app/core/src"))
    inputs.dir(srcRoot.resolve("app/ffi/src"))
    inputs.dir(srcRoot.resolve("rust/src"))
    inputs.files(srcRoot.resolve("app/Cargo.toml"), srcRoot.resolve("app/Cargo.lock"), srcRoot.resolve("rust/Cargo.toml"))
    inputs.file(rootDir.resolve("scripts/build-core.sh"))
    outputs.dir(projectDir.resolve("src/main/jniLibs"))
    outputs.dir(layout.buildDirectory.dir("generated/uniffi"))
}
tasks.named("preBuild") { dependsOn(cargoBuild) }

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.camera.core)
    implementation(libs.androidx.camera.camera2)
    implementation(libs.androidx.camera.lifecycle)
    implementation(libs.androidx.camera.view)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.kotlinx.coroutines.android)
    // uniffi's Kotlin bindings call the Rust library through JNA; the AAR carries libjnidispatch per ABI
    implementation(libs.jna) { artifact { type = "aar" } }
    debugImplementation(libs.androidx.compose.ui.tooling)
}
