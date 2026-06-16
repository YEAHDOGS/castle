plugins {
    kotlin("multiplatform")
}

kotlin {
    jvm()
    
    // Web
    js {
        browser()
        binaries.executable()
    }

    // Android
    // androidTarget()

    // iOS
    // listOf(
    //     iosX64(),
    //     iosArm64(),
    //     iosSimulatorArm64()
    // ).forEach { iosTarget ->
    //     iosTarget.binaries.framework {
    //         baseName = "shared"
    //     }
    // }

    sourceSets {
        val commonMain by getting {
            dependencies {
                implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0")
                // Ktor client
                implementation("io.ktor:ktor-client-core:3.0.0")
            }
        }
        val jvmMain by getting {
            dependencies {
                implementation("io.ktor:ktor-client-cio:3.0.0")
            }
        }
        val jsMain by getting {
            dependencies {
                implementation("io.ktor:ktor-client-js:3.0.0")
            }
        }
    }
}
