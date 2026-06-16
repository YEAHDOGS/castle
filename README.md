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