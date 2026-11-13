// Castle Gatehouse presence node: an ESP32 reads a Hi-Link LD2450 24 GHz radar and tells
// the Castle box what it sees, using this node's own key (never the box token).
//
// NOT YET TESTED ON HARDWARE. It follows the LD2450's published serial protocol and builds
// for the ESP32, but no board has run it. Check the serial monitor the first time.
//
// The LD2450 sends a 30-byte frame about ten times a second:
//   AA FF 03 00 | target 1 (8 bytes) | target 2 | target 3 | 55 CC
// Each target is four little-endian 16-bit numbers: x and y in millimetres, speed in
// centimetres a second, and the distance resolution. For x, y and speed the top bit set
// means positive and clear means negative; the other 15 bits are the size. A target of
// all zeros is an empty slot.
//
// The node sends when what it sees changes, at most every SEND_GAP_MS, and once every
// HEARTBEAT_MS when nothing moves, so the box knows it is still there.

#include <Arduino.h>
#include <HTTPClient.h>
#include <WiFi.h>

#if __has_include("config.h")
#include "config.h"
#else
#error "Copy include/config.example.h to include/config.h and fill it in."
#endif

static const char *FIRMWARE = "gatehouse-ld2450/0.1";
static const uint32_t RADAR_BAUD = 256000;
static const size_t FRAME_LEN = 30;
static const uint8_t HEAD[4] = {0xAA, 0xFF, 0x03, 0x00};
static const uint8_t TAIL[2] = {0x55, 0xCC};
static const int MAX_TARGETS = 3;

static const uint32_t SEND_GAP_MS = 150;     // the box takes at most ~12 a second
static const uint32_t HEARTBEAT_MS = 2000;   // well inside the box's 3 s target life
static const uint32_t REFUSED_WAIT_MS = 30000; // a wrong key: try again rarely
static const int MOVE_MM = 60;               // smaller moves are not worth a report
static const uint32_t WIFI_RETRY_MS = 5000;

struct Target {
  int16_t x, y, speed;
};

static Target seen[MAX_TARGETS];
static int seenCount = 0;
static Target sent[MAX_TARGETS];
static int sentCount = -1; // nothing sent yet
static uint32_t lastSend = 0;
static uint32_t quietUntil = 0;
static uint32_t lastWifiTry = 0;

static uint8_t frame[FRAME_LEN];
static size_t have = 0;

static int16_t signedValue(uint16_t raw) {
  int16_t size = (int16_t)(raw & 0x7FFF);
  return (raw & 0x8000) ? size : -size;
}

static uint16_t word(const uint8_t *p) { return (uint16_t)p[0] | ((uint16_t)p[1] << 8); }

// One whole frame in `frame`: read its targets into `seen`.
static void readFrame() {
  seenCount = 0;
  for (int i = 0; i < MAX_TARGETS; i++) {
    const uint8_t *t = frame + 4 + i * 8;
    bool empty = true;
    for (int b = 0; b < 8; b++) empty &= (t[b] == 0);
    if (empty) continue;
    seen[seenCount++] = {signedValue(word(t)), signedValue(word(t + 2)), signedValue(word(t + 4))};
  }
}

// Feeds bytes from the radar; true when a new frame was read.
static bool pollRadar() {
  bool got = false;
  while (Serial2.available()) {
    uint8_t b = (uint8_t)Serial2.read();
    // Until the four header bytes line up, start again at each byte that could begin one.
    if (have < 4) {
      if (b == HEAD[have]) frame[have++] = b;
      else have = (b == HEAD[0]) ? 1 : 0;
      if (have == 1) frame[0] = HEAD[0];
      continue;
    }
    frame[have++] = b;
    if (have == FRAME_LEN) {
      if (frame[28] == TAIL[0] && frame[29] == TAIL[1]) {
        readFrame();
        got = true;
      }
      have = 0;
    }
  }
  return got;
}

static bool changed() {
  if (sentCount != seenCount) return true;
  for (int i = 0; i < seenCount; i++) {
    if (abs(seen[i].x - sent[i].x) > MOVE_MM || abs(seen[i].y - sent[i].y) > MOVE_MM) return true;
  }
  return false;
}

static void send() {
  String body = "{\"firmware\":\"";
  body += FIRMWARE;
  body += "\",\"targets\":[";
  for (int i = 0; i < seenCount; i++) {
    if (i) body += ',';
    body += "{\"x\":" + String(seen[i].x) + ",\"y\":" + String(seen[i].y) + ",\"speed\":" + String(seen[i].speed) + "}";
  }
  body += "]}";

  HTTPClient http;
  http.setTimeout(2000);
  if (!http.begin(String(BOX_URL) + "/api/v1/presence/report")) return;
  http.addHeader("Content-Type", "application/json");
  http.addHeader("Authorization", String("Node ") + NODE_KEY);
  int status = http.POST(body);
  http.end();

  lastSend = millis();
  if (status == 204) {
    memcpy(sent, seen, sizeof(seen));
    sentCount = seenCount;
  } else if (status == 401) {
    Serial.println("The box refused this node's key. Add the node again or choose New key in the app.");
    quietUntil = millis() + REFUSED_WAIT_MS;
  } else if (status == 429) {
    quietUntil = millis() + SEND_GAP_MS * 2;
  } else {
    Serial.printf("The box answered %d\n", status);
  }
}

static void keepWifi() {
  if (WiFi.status() == WL_CONNECTED) return;
  if (millis() - lastWifiTry < WIFI_RETRY_MS) return;
  lastWifiTry = millis();
  Serial.println("Joining Wi-Fi...");
  WiFi.disconnect();
  WiFi.begin(WIFI_SSID, WIFI_PASSWORD);
}

void setup() {
  Serial.begin(115200);
  Serial2.begin(RADAR_BAUD, SERIAL_8N1, RADAR_RX, RADAR_TX);
  WiFi.mode(WIFI_STA);
  WiFi.setSleep(false); // a sleeping radio makes reports late
  keepWifi();
  Serial.printf("%s starting\n", FIRMWARE);
}

void loop() {
  pollRadar();
  keepWifi();
  if (WiFi.status() != WL_CONNECTED) return;
  uint32_t now = millis();
  if ((int32_t)(now - quietUntil) < 0) return;
  if (now - lastSend < SEND_GAP_MS) return;
  if (changed() || now - lastSend >= HEARTBEAT_MS) send();
}
