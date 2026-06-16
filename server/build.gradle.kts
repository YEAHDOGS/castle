plugins {
    kotlin("jvm")
    id("io.ktor.plugin")
}

group = "com.castle"
version = "1.0-SNAPSHOT"

application {
    mainClass.set("com.castle.server.ApplicationKt")
}

dependencies {
    implementation("io.ktor:ktor-server-core-jvm")
    implementation("io.ktor:ktor-server-netty-jvm")
    implementation("ch.qos.logback:logback-classic:1.4.14")
}
