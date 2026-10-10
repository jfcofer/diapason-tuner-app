// The Android side of core_platform, from Flutter 3.47's plugin template. SDK levels come from
// tools/versions.env, as the app's do, so a bump stays one edit in one file (AGENTS.md §8).

group = "dev.jfcofer.diapason.core_platform"
version = "1.0-SNAPSHOT"

buildscript {
    val kotlinVersion = "2.4.0"
    repositories {
        google()
        mavenCentral()
    }

    dependencies {
        classpath("com.android.tools.build:gradle:9.1.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:$kotlinVersion")
    }
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

plugins {
    id("com.android.library")
}

val pinned: Map<String, String> =
    file("../../../tools/versions.env")
        .readLines()
        .filter { it.contains("=") && !it.trimStart().startsWith("#") }
        .associate { line -> line.substringBefore("=").trim() to line.substringAfter("=").trim() }

android {
    namespace = "dev.jfcofer.diapason.core_platform"

    compileSdk = pinned.getValue("ANDROID_COMPILE_SDK").toInt()

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets { getByName("main") { java.srcDirs("src/main/kotlin") } }

    defaultConfig { minSdk = pinned.getValue("ANDROID_MIN_SDK").toInt() }
}

kotlin { compilerOptions { jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17 } }
