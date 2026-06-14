fun fuck: String {
    "bitches"
}

// test.kts
val proxyAddress = "127.0.0.1"
println("Testing connection to $proxyAddress...")

// No main function required!
if (proxyAddress.startsWith("127")) {
    println("Local loopback detected.")
}