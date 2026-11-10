//! Network: what the box can see of its own connection today.
//!
//! * Live traffic: byte counters for each network adapter (the app turns two readings into
//!   a rate).
//! * Local details: this machine's address, its gateway (the router) and DNS servers.
//! * A speed test (against Cloudflare's public speed-test service) and an ISP lookup
//!   (ipinfo.io). Both reach out to the internet, and only when someone asks for them.
//! * Whether Tailscale or WireGuard is installed, and Tailscale's state.
//!
//! Castle is not the router yet, so this sees the box's own traffic, not the whole
//! network's. Per-device traffic, DNS filtering and ad blocking need the box to sit in
//! the path, which is the Gateway work still ahead.

use std::{
    path::Path,
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sysinfo::Networks;

const SPEED_HOST: &str = "https://speed.cloudflare.com";
const DOWNLOAD_BYTES: u64 = 50_000_000;
const UPLOAD_BYTES: usize = 10_000_000;
const LATENCY_SAMPLES: usize = 4;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(40);
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);
const PROVIDER_URL: &str = "https://ipinfo.io/json";
const TRACE_URL: &str = "https://1.1.1.1/cdn-cgi/trace";
const USER_AGENT: &str = concat!("castled/", env!("CARGO_PKG_VERSION"));
const LOCAL_CACHE: Duration = Duration::from_secs(60);
const BITS_PER_MEGABIT: f64 = 1_000_000.0;
const TAILSCALE_PATHS: [&str; 2] = ["tailscale", r"C:\Program Files\Tailscale\tailscale.exe"];
const WIREGUARD_PATHS: [&str; 2] = [r"C:\Program Files\WireGuard\wireguard.exe", "/usr/bin/wg"];
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    name: String,
    /// Bytes received and sent since the machine started.
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Local {
    adapter: String,
    address: String,
    gateway: String,
    dns: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// When the counters were read, in milliseconds since 1970.
    at: u64,
    interfaces: Vec<Interface>,
    local: Local,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeedResult {
    at: u64,
    down_mbps: f64,
    up_mbps: f64,
    latency_ms: f64,
    download_bytes: u64,
    upload_bytes: u64,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Tailscale {
    installed: bool,
    /// Tailscale's own word for its state (for example Running, NeedsLogin, Stopped), or
    /// empty when it could not be read.
    state: String,
    name: String,
    addresses: Vec<String>,
    tailnet: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Links {
    tailscale: Tailscale,
    wireguard_installed: bool,
}

pub struct Network {
    local: Mutex<Option<(Instant, Local)>>,
    /// Held for the length of a speed test, so two never run at once and skew each other.
    testing: Mutex<()>,
}

pub type NetworkResult<T> = Result<T, String>;

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn quiet(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null()).stderr(Stdio::null());
    cmd
}

fn http(timeout: Duration) -> NetworkResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| "could not start a network client".to_string())
}

/// Megabits per second for `bytes` moved in `elapsed`, to one decimal place.
fn mbps(bytes: u64, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.0 {
        return 0.0;
    }
    (bytes as f64 * 8.0 / seconds / BITS_PER_MEGABIT * 10.0).round() / 10.0
}

/// Splits "AS64500 Example Fiber" into the network number and the provider's name.
fn split_org(org: &str) -> (String, String) {
    match org.split_once(' ') {
        Some((asn, name)) if asn.starts_with("AS") && asn[2..].chars().all(|c| c.is_ascii_digit()) => (asn.to_string(), name.to_string()),
        _ => (String::new(), org.to_string()),
    }
}

#[cfg(windows)]
fn read_local() -> Local {
    let script = "$c = Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway } | Select-Object -First 1; \
                  if ($c) { $c.InterfaceAlias; $c.IPv4Address.IPAddress | Select-Object -First 1; $c.IPv4DefaultGateway.NextHop | Select-Object -First 1; \
                  ($c.DNSServer | Where-Object { $_.AddressFamily -eq 2 }).ServerAddresses -join ',' }";
    let Ok(out) = quiet("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).output() else {
        return Local::default();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines().map(str::trim);
    Local {
        adapter: lines.next().unwrap_or("").to_string(),
        address: lines.next().unwrap_or("").to_string(),
        gateway: lines.next().unwrap_or("").to_string(),
        dns: lines.next().unwrap_or("").split(',').filter(|s| !s.is_empty()).map(str::to_string).collect(),
    }
}

#[cfg(not(windows))]
fn read_local() -> Local {
    // The resolver list is the one detail that is the same on every Linux box.
    let dns = std::fs::read_to_string("/etc/resolv.conf")
        .map(|text| text.lines().filter_map(|l| l.strip_prefix("nameserver ")).map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();
    Local { dns, ..Local::default() }
}

fn tailscale() -> Tailscale {
    for exe in TAILSCALE_PATHS {
        let Ok(out) = quiet(exe).args(["status", "--json"]).output() else { continue };
        // The program exists. Its JSON layout is not a documented contract, so every field
        // is read as optional and a missing one just leaves that detail blank.
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
        let text = |v: &serde_json::Value| v.as_str().unwrap_or("").trim_end_matches('.').to_string();
        return Tailscale {
            installed: true,
            state: text(&json["BackendState"]),
            name: text(&json["Self"]["DNSName"]),
            addresses: json["Self"]["TailscaleIPs"]
                .as_array()
                .map(|ips| ips.iter().filter_map(|ip| ip.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            tailnet: text(&json["CurrentTailnet"]["Name"]),
        };
    }
    Tailscale::default()
}

impl Network {
    pub fn new() -> Self {
        Self { local: Mutex::new(None), testing: Mutex::new(()) }
    }

    fn local(&self) -> Local {
        let mut cached = self.local.lock().unwrap();
        if let Some((at, local)) = cached.as_ref() {
            if at.elapsed() < LOCAL_CACHE {
                return local.clone();
            }
        }
        let local = read_local();
        *cached = Some((Instant::now(), local.clone()));
        local
    }

    /// Adapter byte counters right now. Adapters that have never carried traffic are left out.
    pub fn reading(&self) -> Reading {
        // No adapter list (the operating system refused) reads as "no adapters", not an error.
        let networks = Networks::new_with_refreshed_list().ok();
        let mut interfaces: Vec<Interface> = networks
            .iter()
            .flatten()
            .map(|(name, data)| Interface { name: name.to_string_lossy().into_owned(), rx_bytes: data.total_received(), tx_bytes: data.total_transmitted() })
            .filter(|i| i.rx_bytes > 0 || i.tx_bytes > 0)
            .collect();
        interfaces.sort_by(|a, b| (b.rx_bytes + b.tx_bytes).cmp(&(a.rx_bytes + a.tx_bytes)));
        Reading { at: now_ms(), interfaces, local: self.local() }
    }

    /// Measures latency, then download, then upload, against Cloudflare's speed-test
    /// service. Download and upload are averages over the whole transfer on one
    /// connection, so on a very fast line they read lower than a multi-stream test would.
    pub fn speed_test(&self) -> NetworkResult<SpeedResult> {
        let Ok(_running) = self.testing.try_lock() else {
            return Err("a speed test is already running".into());
        };
        let unreachable = |_| "could not reach the speed-test service".to_string();
        let client = http(TRANSFER_TIMEOUT)?;

        // Latency: the fastest of a few tiny requests on the already-open connection. The
        // first request also pays for the TLS handshake, so it is sent but not counted.
        let ping_url = format!("{SPEED_HOST}/__down?bytes=0");
        client.get(&ping_url).send().and_then(|r| r.error_for_status()).map_err(unreachable)?;
        let mut latency_ms = f64::INFINITY;
        for _ in 0..LATENCY_SAMPLES {
            let started = Instant::now();
            client.get(&ping_url).send().and_then(|r| r.bytes()).map_err(unreachable)?;
            latency_ms = latency_ms.min(started.elapsed().as_secs_f64() * 1000.0);
        }

        let started = Instant::now();
        let mut response = client
            .get(format!("{SPEED_HOST}/__down?bytes={DOWNLOAD_BYTES}"))
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(unreachable)?;
        let downloaded = std::io::copy(&mut response, &mut std::io::sink()).map_err(|_| "the download test was cut short".to_string())?;
        let down_mbps = mbps(downloaded, started.elapsed());

        let started = Instant::now();
        client
            .post(format!("{SPEED_HOST}/__up"))
            .header("Content-Type", "application/octet-stream")
            .body(vec![0u8; UPLOAD_BYTES])
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(unreachable)?;
        let up_mbps = mbps(UPLOAD_BYTES as u64, started.elapsed());

        Ok(SpeedResult {
            at: now_ms(),
            down_mbps,
            up_mbps,
            latency_ms: (latency_ms * 10.0).round() / 10.0,
            download_bytes: downloaded,
            upload_bytes: UPLOAD_BYTES as u64,
        })
    }

    /// Who the internet provider is, as seen from outside: public address, provider name
    /// and rough location. This asks ipinfo.io, which necessarily sees the public address;
    /// it runs only when asked for.
    pub fn provider(&self) -> NetworkResult<serde_json::Value> {
        let client = http(LOOKUP_TIMEOUT)?;
        let info = client
            .get(PROVIDER_URL)
            .send()
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.json::<serde_json::Value>());
        if let Ok(info) = info {
            if info["ip"].is_string() {
                let (asn, name) = split_org(info["org"].as_str().unwrap_or(""));
                return Ok(serde_json::json!({
                    "address": info["ip"],
                    "provider": name,
                    "asn": asn,
                    "city": info["city"],
                    "region": info["region"],
                    "country": info["country"],
                    "timezone": info["timezone"],
                }));
            }
        }
        // Fallback: Cloudflare's trace page gives the address and country, but no provider name.
        let trace = client
            .get(TRACE_URL)
            .send()
            .and_then(|r| r.text())
            .map_err(|_| "could not reach the lookup service".to_string())?;
        let field = |name: &str| trace.lines().find_map(|l| l.strip_prefix(&format!("{name}="))).unwrap_or("").to_string();
        Ok(serde_json::json!({ "address": field("ip"), "provider": "", "asn": "", "country": field("loc") }))
    }

    pub fn links(&self) -> Links {
        Links { tailscale: tailscale(), wireguard_installed: WIREGUARD_PATHS.iter().any(|p| Path::new(p).is_file()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speeds_are_megabits_per_second() {
        assert_eq!(mbps(12_500_000, Duration::from_secs(1)), 100.0);
        assert_eq!(mbps(50_000_000, Duration::from_secs(4)), 100.0);
        assert_eq!(mbps(1_000, Duration::ZERO), 0.0);
    }

    #[test]
    fn provider_names_are_split_from_their_network_number() {
        assert_eq!(split_org("AS64500 Example Fiber LLC"), ("AS64500".to_string(), "Example Fiber LLC".to_string()));
        assert_eq!(split_org("Example Fiber"), (String::new(), "Example Fiber".to_string()));
        assert_eq!(split_org(""), (String::new(), String::new()));
    }
}
