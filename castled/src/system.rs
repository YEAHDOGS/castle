//! The box's vital signs: processor, memory, the vault's disk, and how long the machine
//! has been up. Read-only, and about this machine only.

use std::{
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vitals {
    /// Milliseconds since 1970, by the box's clock.
    at: u64,
    host: String,
    os: String,
    uptime_seconds: u64,
    cores: usize,
    /// Processor use across all cores since the previous reading, 0 to 100. The first
    /// reading after the box starts has nothing to compare with and reports 0.
    cpu_percent: f32,
    memory_used: u64,
    memory_total: u64,
    /// The disk that holds the vault.
    disk_free: u64,
    disk_total: u64,
}

pub struct System {
    /// Kept between readings: processor use is the change from one refresh to the next.
    /// None when this machine's readings could not be opened at all.
    sys: Mutex<Option<sysinfo::System>>,
}

impl System {
    pub fn new() -> Self {
        let mut sys = sysinfo::System::new().ok();
        if let Some(sys) = sys.as_mut() {
            sys.refresh_cpu_usage();
        }
        Self { sys: Mutex::new(sys) }
    }

    pub fn vitals(&self, vault: &Path) -> Result<Vitals, String> {
        const UNAVAILABLE: &str = "vital signs are unavailable on this machine";
        let mut guard = self.sys.lock().map_err(|_| UNAVAILABLE.to_string())?;
        let sys = guard.as_mut().ok_or(UNAVAILABLE)?;
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Ok(Vitals {
            at: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
            host: sysinfo::System::host_name().unwrap_or_default(),
            os: sysinfo::System::long_os_version().unwrap_or_default(),
            uptime_seconds: sysinfo::System::uptime().unwrap_or(0),
            cores: sys.cpus().len(),
            cpu_percent: sys.global_cpu_usage().clamp(0.0, 100.0),
            memory_used: sys.used_memory(),
            memory_total: sys.total_memory(),
            disk_free: fs4::available_space(vault).unwrap_or(0),
            disk_total: fs4::total_space(vault).unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vitals_are_plausible() {
        let vitals = System::new().vitals(&std::env::temp_dir()).unwrap();
        assert!(vitals.cores >= 1);
        assert!(vitals.memory_total >= vitals.memory_used && vitals.memory_total > 0);
        assert!(vitals.disk_total >= vitals.disk_free && vitals.disk_total > 0);
        assert!((0.0..=100.0).contains(&vitals.cpu_percent));
    }
}
