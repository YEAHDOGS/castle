package com.castle.server

import io.ktor.server.application.*
import io.ktor.server.engine.*
import io.ktor.server.netty.*
import io.ktor.server.response.*
import io.ktor.server.routing.*

fun main() {
    embeddedServer(Netty, port = 8080, host = "0.0.0.0", module = Application::module)
        .start(wait = true)
}

fun Application.module() {
    routing {
        get("/") {
            call.respondText("Castle Cloud Server is running!")
        }

        route("/sync") {
            // Check file metadata/timestamp
            get("/status/{filename}") {
                val filename = call.parameters["filename"]
                // TODO: Return timestamp from SQLite DB or file system
                call.respondText("Status for $filename")
            }

            // Download file
            get("/download/{filename}") {
                val filename = call.parameters["filename"]
                // TODO: Serve the file from the SD card mount
                call.respondText("Downloading $filename")
            }

            // Upload file
            post("/upload/{filename}") {
                val filename = call.parameters["filename"]
                // TODO: Receive file content and write to SD card mount
                call.respondText("Uploaded $filename")
            }
        }
    }
}
