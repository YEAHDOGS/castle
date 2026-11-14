# Castle 🏰

A way to make data yours again


[ THE OUTSIDE WORLD ]
                                                │
         ┌──────────────────────────────────────┼──────────────────────────────────────┐
         ▼                                      ▼                                      ▼
   ( Web Browser )                       ( Native Client App )                  ( Off-Grid Radio )
   [ Desktop/Laptop ]                    [ Android / iOS / Desktop ]            [ Portable LoRa Node ]
         │                                      │                                      │
         │ (Public HTTPS)                       │ (Encrypted WireGuard/Tailscale)      │ (915MHz Radio Waves)
         ▼                                      ▼                                      ▼
   [ CASTLE PORTAL ]                    [ CASTLE DRAWBRIDGE ]                  [ DISTRIBUTED NODES ]
   (Cloudflare Tunnel / Reverse Proxy)  (VPN Gateway Ingress)                  (Remote P2P Mesh)
         │                                      │                                      │
         └──────────────────────┬───────────────┘                                      │
                                │                                                      │
                                ▼                                                      ▼
                  [ CASTLE GATEHOUSE (Router) ] ◄──────────────────────────────[ BASE STATION NODE ]
                  (Edge Hardware / Packet Management)                           (USB Serial / API Bridge)
                                │
                                ▼
                  ===========================================
                  [               THE ARMOURY               ]
                  [     (Perimeter Firewall & Security)     ]
                  ===========================================
                                │
                                ├──► [ PI-HOLE DNS ] ──► (Drops Ads, Tracker Logs, & Malicious Domains)
                                ├──► [ REVERSE PROXY ] ──► (SSL Termination & Fine-Grained Port Mapping)
                                └──► [ BLOCKCHAIN IMMUTABLE LEDGER ] ──► (Cryptographic Access Signing)
                                │
                                ▼
                    =========================
                    [ THE INNER COURTYARD ]
                    [  (Core Network Grid)  ]
                    =========================
                                │
         ┌──────────────────────┼──────────────────────┐
         ▼                      ▼                      ▼
  [ THE COMPUTE CORE ]   [ THE DATA VAULT ]     [ THE STABLES ]
  (The Armory Room)      (The Keep / Storage)   (Compute Node Parking)
  ┌──────────────────┐   ┌──────────────────┐   ┌──────────────────────┐
  │ • Distributed    │   │ • Samba Shares   │   │ • Repurposed Towers  │
  │   GPU Hardware   │   │ • Nextcloud /    │   │ • Android Mobile Labs│
  │ • Local LLMs     │   │   Owncloud Core  │   │ • Isolated Sanboxes  │
  │ • Autonomous AI  │   │ • Sync Daemons   │   │ • 24/7 Scraping &    │
  │   Agent Engines  │   │   (Photos/Cams)  │   │   Compilation Hubs   │
  └──────────────────┘   └──────────────────┘   └──────────────────────┘
                                ▲
                                │
                  ===============================
                  [ IDENTITY & ACCESS CONTROL ]
                  [ (User Profiles & Identity)  ]
                  ===============================
                                │
         ┌──────────────────────┴──────────────────────┐
         ▼                                             ▼
  [ ADMIN PROFILE ]                             [ USER PROFILES ]
  (Full Root Access to Nodes)                  (Restricted Read/Write Context)
  • Storage Array Shell Administration          • Access to Personal Storage Buckets
  • Firewall Ruleset Modification               • Basic Compute Allocation Access
  • Global Ledger Overwatch                      • No Root System File Permissions
---

## Layout

Four folders at the top, and everything that is part of the app lives in `app/`.

| Folder | What it is |
|---|---|
| `app/` | **The Castle app**: the interface (Svelte, `app/src`), its desktop and phone shell (Tauri, `app/src-tauri`), and everything the app runs on. |
| `app/castled/` | The box daemon (Rust). Storage, Arcade, Theater, Comms, Contacts, Clones, Security and Presence all answer from here; the desktop app carries it inside. |
| `app/ai/` | DOGS AI, which the AI section shows: its app, its server and the scripts that install its runtime. |
| `app/clonewars/` | The Clones engine: QEMU virtual machines, Docker containers and the Android emulator, with the automations that drive them. |
| `app/security/` | The audit scripts behind Security > Audits. The daemon builds them in. |
| `app/gatehouse/` | Firmware for the Presence radar nodes (ESP32 + LD2450). |
| `app/data/` | Everything live or generated, never committed: Clones' disks, ISOs and Android SDK (`app/data/clonewars`), DOGS AI's runtime, models and outputs (`app/data/ai`), and the vault a plain `cargo run` of the daemon uses (`app/data/castle`). |
| `os/` | Castle OS: the Arch-based install image and its tools. |
| `landing/` | castle.dogs.red, the landing page. |
| `scripts/` | Repo scripts, such as building the website. |

`start.sh` (or `start.cmd` on Windows) runs the whole thing for development: the daemon, the app and DOGS AI, rebuilding each as you edit it.

## The website and its demo

castle.dogs.red is the landing page (`landing/`) with a read-only demo of the whole app at
`/demo/`: every section with sample data, and every button that would run or change something
says so instead. It is the app built with `npm run build:showcase` (see
`app/src/lib/showcase.js`).

The site is the Cloudflare Pages project `castle`, connected to this repo: **every push to
`master` builds and publishes it**, and other branches get preview addresses. The build is
`node scripts/build-site.mjs`, which puts the landing page and the demo together in `site-dist/`.
Run it yourself to see the site before you push.
