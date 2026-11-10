package com.castle.server

import io.ktor.http.*
import io.ktor.server.application.*
import io.ktor.server.engine.*
import io.ktor.server.netty.*
import io.ktor.server.response.*
import io.ktor.server.routing.*
import java.io.File

// Storage root for the Data Vault -- the "self-hosted Google Drive" directory
// this box serves on the LAN. Override with CASTLE_DATA_DIR; the future
// warehouse offsite tier replicates this directory, encrypted.
val dataDir: File = File(System.getenv("CASTLE_DATA_DIR") ?: "./data").apply { mkdirs() }

/**
 * Rejects anything that could escape the vault root once real file serving
 * lands: path traversal (".."), absolute paths, separators, overlong names.
 * Returns the clean filename, or null when the input is unsafe.
 */
fun sanitizeFileName(raw: String?): String? {
    if (raw.isNullOrBlank()) return null
    if (raw.length > 255) return null
    if (raw.contains("..") || raw.contains('/') || raw.contains('\\')) return null
    if (raw.startsWith(".")) return null
    return raw
}

/**
 * True only for a regular file that really lives inside the vault root: a
 * symlink (or junction) planted in the vault must not serve files outside it.
 */
fun isInsideVault(file: File): Boolean {
    if (java.nio.file.Files.isSymbolicLink(file.toPath())) return false
    val root = dataDir.canonicalFile.toPath()
    return file.isFile && file.canonicalFile.toPath().startsWith(root)
}

// No auth exists yet, so the server answers on this machine only unless
// CASTLE_BIND says otherwise. Do not set it to 0.0.0.0 on a shared network.
val bindHost: String = System.getenv("CASTLE_BIND") ?: "127.0.0.1"

fun main() {
    embeddedServer(Netty, port = 8080, host = bindHost, module = Application::module)
        .start(wait = true)
}

fun Application.module() {
    routing {
        get("/") {
            call.respondText("Castle Cloud Server is running!")
        }

        // Liveness probe for the Drawbridge / LAN health checks.
        get("/health") {
            val files = dataDir.listFiles()?.size ?: 0
            call.respondText("{\"status\":\"ok\",\"vault_files\":$files}", ContentType.Application.Json)
        }

        route("/sync") {
            // Check file metadata/timestamp
            get("/status/{filename}") {
                val filename = sanitizeFileName(call.parameters["filename"])
                if (filename == null) {
                    call.respondText("Invalid filename", status = HttpStatusCode.BadRequest)
                    return@get
                }
                // TODO: return last-modified + size from the vault (SQLite
                // index or filesystem metadata) as JSON.
                call.respondText("Status for $filename")
            }

            // Download file
            get("/download/{filename}") {
                val filename = sanitizeFileName(call.parameters["filename"])
                if (filename == null) {
                    call.respondText("Invalid filename", status = HttpStatusCode.BadRequest)
                    return@get
                }
                val file = File(dataDir, filename)
                if (!isInsideVault(file)) {
                    call.respondText("Not found", status = HttpStatusCode.NotFound)
                    return@get
                }
                // TODO: stream with Content-Disposition + resume (Range) support.
                call.respondFile(file)
            }

            // Upload file
            post("/upload/{filename}") {
                val filename = sanitizeFileName(call.parameters["filename"])
                if (filename == null) {
                    call.respondText("Invalid filename", status = HttpStatusCode.BadRequest)
                    return@post
                }
                // TODO: stream the request body to dataDir/filename (write to
                // a temp file + atomic rename), then update the index.
                // Not built yet: say so instead of reporting a write that never happened.
                call.respondText("Upload is not implemented yet", status = HttpStatusCode.NotImplemented)
            }
        }
    }
}
