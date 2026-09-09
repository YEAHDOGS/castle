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

## Components

| Directory | What it is |
|---|---|
| `clonewars/` | **Clone Factory** — modular PowerShell QEMU pipeline: download, cryptographically verify (hash + GPG + trust pinning), provision, and boot VM targets (CachyOS, Ubuntu, Windows, …). Interactive CLI (`start.ps1`), WPF GUI (`gui.ps1`), Docker + noVNC support. See `clonewars/README.md`. |
| `vault/` | **Family Data Vault** — per-user encrypted vaults, age-tiered accounts, sharing ladder, receipt ingestion (email/POS/scan), manifest + drift audit, encrypted-at-rest sealing. See `vault/README.md` and `docs/FAMILY-DATA-VAULT.md`. |
| `flamethrower/` | **True-deletion tooling** — Tier-1 crypto-shred keyring, Tier-2 firmware erase, Tier-3 file shredder, residue wiping, deletion certificates + hash-chained audit log, unified CLI. See `flamethrower/README.md` and `docs/FLAMETHROWER.md`. |
| `backup/` | **Backup intake** — Castle's receiving end for machine backups (Phoenix emergency runbook's 10TB target): quarantine-aware intake with SHA-256 manifest + verify. See `backup/README.md` and `docs/BACKUP-INTAKE.md`. |
| `arcade/` | **Retro asset pipeline** — acquire, hash-verify, extract, and organize console firmware/BIOS + ROM images into `arcade/data/`. See `arcade/README.md`. |
| `saves/` | **SaveVault → Chains** — moved out of castle into its own standalone project ("git for video game save data": version control for emulator save files — SNES `.srm`, GBA `.sav`, save states). See the `chains` project. |
| `server/` | **Castle sync server** (Kotlin/Ktor) — the Data Vault backend: file sync endpoints, vault storage root via `CASTLE_DATA_DIR`. |
| `app/` | **Sync client** (Kotlin multiplatform) — talks to the server; point `baseUrl` at your Castle box on the LAN. |
| `swarm/` | Agent-team design notes. |

**The idea:** Castle is a router replacement with built-in local network storage — a self-hosted Google Drive on your own LAN. Clonewars builds the compute nodes, arcade + saves cover the retro-gaming thread, and the sync server/app are the storage heart. Future: warehouse facilities for encrypted offsite backup.
