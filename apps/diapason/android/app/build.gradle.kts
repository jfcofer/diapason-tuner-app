plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// SDK and NDK versions are pinned in tools/versions.env, which tools/doctor.sh enforces and CI
// reads. They are read from there rather than duplicated, so a bump is one edit in one file
// (AGENTS.md §8: bumping any of these is its own task and its own ADR).
val pinned: Map<String, String> = rootProject.file("../../../tools/versions.env")
    .readLines()
    .filter { it.contains("=") && !it.trimStart().startsWith("#") }
    .associate { line -> line.substringBefore("=").trim() to line.substringAfter("=").trim() }

android {
    namespace = "dev.jfcofer.diapason"
    compileSdk = pinned.getValue("ANDROID_COMPILE_SDK").toInt()
    ndkVersion = pinned.getValue("ANDROID_NDK_VERSION")

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "dev.jfcofer.diapason"
        minSdk = pinned.getValue("ANDROID_MIN_SDK").toInt()
        targetSdk = pinned.getValue("ANDROID_TARGET_SDK").toInt()
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    // Three flavours that install side by side (docs/CI_RELEASE.md §2). Each supplies its own
    // app_name from src/<flavour>/res/, with a values-es/ variant, so the launcher label is both
    // flavour-aware and locale-aware: the Spanish launcher reads "Diapasón" (docs/adr/0013).
    flavorDimensions += "environment"
    productFlavors {
        create("dev") {
            dimension = "environment"
            applicationIdSuffix = ".dev"
        }
        create("stg") {
            dimension = "environment"
            applicationIdSuffix = ".stg"
        }
        create("prod") {
            dimension = "environment"
        }
    }

    buildTypes {
        release {
            // TODO(T-0xx): real signing config, milestone M7. Debug keys keep
            // `flutter run --release` working until then; CI builds unsigned artifacts only.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}
