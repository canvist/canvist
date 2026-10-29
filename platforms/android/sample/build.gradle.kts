plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

/** The Gradle property [property], else the environment variable [variable], else empty. */
fun setting(
    property: String,
    variable: String,
): String =
    providers
        .gradleProperty(property)
        .orElse(providers.environmentVariable(variable))
        .getOrElse("")

/** [value] as a Java string literal. */
fun quoted(value: String) = "\"" + value.replace("\\", "\\\\").replace("\"", "\\\"") + "\""

android {
    namespace = "io.github.ifiokjr.canvist.sample"
    compileSdk = 37

    defaultConfig {
        applicationId = "io.github.ifiokjr.canvist.sample"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // Only these ABIs have a `libcanvist_ffi.so`; without the filter JNA's
        // AAR would package `libjnidispatch.so` for five more and the app would
        // install, then fail to load Rust, on those devices.
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }

        // Dictation goes through a relay when both are set, as Gradle
        // properties (`-PcanvistDictationUrl=wss://… -PcanvistDictationToken=…`)
        // or as `CANVIST_DICTATION_URL` and `CANVIST_DICTATION_TOKEN` in the
        // environment. The token is for trying a relay out; a real app fetches a
        // short-lived one from its own server.
        val dictationUrl = setting("canvistDictationUrl", "CANVIST_DICTATION_URL")
        val dictationToken = setting("canvistDictationToken", "CANVIST_DICTATION_TOKEN")
        buildConfigField("String", "DICTATION_URL", quoted(dictationUrl))
        buildConfigField("String", "DICTATION_TOKEN", quoted(dictationToken))
    }

    buildFeatures {
        buildConfig = true
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation(project(":canvist"))
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.material3)

    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.test.espresso.core)
    androidTestImplementation(libs.androidx.test.runner)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
}
