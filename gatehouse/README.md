# Gatehouse: presence nodes

Small radar nodes that tell the Castle box when something is moving in a room or through a
doorway. The app draws them as a motion-tracker sweep (Security > Presence) and keeps a
list of rooms, occupied or empty, with when each was entered and left.

There is no camera. The box senses; it does not watch.

> **Status: the firmware has not been run on hardware yet.** It follows the LD2450's
> published serial protocol and is written for a stock ESP32 dev board, but nobody has
> wired one up. The box side (`castled/src/presence.rs`) and the app side are built and
> tested; the Demo shows what it will look like.

## What one node is

| Part | What it does | Roughly |
|---|---|---|
| ESP32 dev board (ESP32-WROOM-32, "DevKitC") | Reads the radar, sends to the box over Wi-Fi | $4–8 |
| Hi-Link **LD2450** 24 GHz mmWave sensor | Tracks up to three targets by position (x, y) and speed | $6–12 |
| USB power supply and cable | 5 V for both | whatever is in the drawer |

About **$10–20 a node**. An **LD2410** works too, for a node that only needs to say
"someone is here" (no positions, so no blips on the sweep); choose LD2410 when adding the
node. This firmware reads the LD2450; an LD2410 node needs its own small sketch that sends
`{"present": true}` or `false`.

## Wiring

| LD2450 | ESP32 |
|---|---|
| 5V | 5V (VIN) |
| GND | GND |
| TX | GPIO 16 (RX2) |
| RX | GPIO 17 (TX2) |

The LD2450 talks at 256000 baud; the pins are set in `include/config.h`.

Mount it about chest height, facing into the room or across the doorway. It sees roughly
6 m out and about 60° to each side.

## Pairing a node with the box

1. The box has to listen on the home network, not only on its own computer: start it with
   `castled --bind 0.0.0.0:8787` (or the address of the box on your network). A box that
   listens only on 127.0.0.1 refuses every node.
2. In the Castle app, as an admin: **Security > Presence > Add a node**. Give it a name
   ("Hall door") and a room ("Hall"). The app shows the **node key once**. Copy it.
3. Copy `node/include/config.example.h` to `node/include/config.h` and fill in your Wi-Fi,
   the box's address and the node key.
4. Flash it with [PlatformIO](https://platformio.org/): from `gatehouse/node`, run
   `pio run -t upload`, then `pio device monitor` to watch it join Wi-Fi.
5. The node turns **online** in the app within a few seconds. Walk past it.

A node's key can report what that node sees and **nothing else**. It cannot read files,
change settings or see other nodes, so a board on the wall never holds the box token. If a
node is lost or its key leaks, choose **New key** (the old key stops working at once) or
remove the node.

## What it sends

```
POST /api/v1/presence/report
Authorization: Node <node key>
Content-Type: application/json

{"firmware":"gatehouse-ld2450/0.1","targets":[{"x":-420,"y":1830,"speed":-12}]}
```

`x` is millimetres across (negative is to the sensor's left), `y` millimetres straight out,
`speed` centimetres a second (negative is coming closer). At most three targets. A node
without positions sends `{"present": true}`. The box answers 204, or 401 for a wrong key,
429 when reports come faster than about 12 a second, and 400 for a reading no sensor could
make. A report is at most 2 KB.

The node sends when what it sees changes (at most every 150 ms) and every 2 s when nothing
does. The box forgets a reading after 3 s and calls a node offline after 30 s of quiet.
Readings live in memory only; a restart of the box forgets where everyone was.

## Honest limits

- **Presence is not identity.** It says something alive is there, not who. A person and a
  large dog look much the same.
- **Pets, fans, curtains and plants in a draught** show up until the sensor is tuned. Hi-Link's
  phone app (HLKRadarTool, over Bluetooth) sets the sensitivity and can mask out areas.
- **Walls.** 24 GHz passes wood, drywall and frosted glass, weaker each time; brick, concrete
  and metal mostly stop it. This is an indoor early-warning sense, not a perimeter fence.
- **Plain HTTP.** The node talks to the box over plain HTTP on the home network, like the
  rest of Castle today. Someone who can watch your network's traffic could read the
  readings and copy a node's key, and with it send false readings for that one node (and
  nothing more). Replace the key if you suspect that.
- **One sensor sees a cone, not a room.** A big room or an L-shaped one needs two nodes; put
  them in the same room in the app and the room counts as occupied when either sees someone.
