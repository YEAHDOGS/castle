package com.castle.sync

import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*

class SyncClient(
    // Point this at the Castle box on your LAN (or the Drawbridge VPN
    // address) instead of localhost when the server runs elsewhere.
    private val baseUrl: String = "http://localhost:8080/sync"
) {
    private val client = HttpClient()

    suspend fun getStatus(filename: String): String {
        return client.get("$baseUrl/status/$filename").bodyAsText()
    }

    suspend fun uploadFile(filename: String, content: ByteArray) {
        client.post("$baseUrl/upload/$filename") {
            setBody(content)
        }
    }

    suspend fun downloadFile(filename: String): ByteArray {
        return client.get("$baseUrl/download/$filename").readBytes()
    }
}
