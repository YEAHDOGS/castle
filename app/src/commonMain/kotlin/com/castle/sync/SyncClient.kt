package com.castle.sync

import io.ktor.client.*
import io.ktor.client.request.*
import io.ktor.client.statement.*

class SyncClient {
    private val client = HttpClient()
    private val baseUrl = "http://localhost:8080/sync"

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
