// Copy this file to config.h (which git ignores) and fill it in. config.h holds the
// node key, so keep it off anything you share.

#pragma once

// Your home Wi-Fi. The ESP32 speaks 2.4 GHz only.
#define WIFI_SSID "your-network"
#define WIFI_PASSWORD "your-password"

// The Castle box, as this node reaches it over the home network. The box has to be
// listening on the network for this to work (castled --bind 0.0.0.0:8787), not only on
// its own computer.
#define BOX_URL "http://192.168.1.20:8787"

// The node key the Castle app showed when you added this node (Security > Presence >
// Add a node). It is shown once; if it is lost, choose New key in the app and paste the
// new one here.
#define NODE_KEY "paste-the-node-key-here"

// Where the LD2450 is wired. Its TX goes to RADAR_RX on the ESP32, its RX to RADAR_TX.
#define RADAR_RX 16
#define RADAR_TX 17
