//! Live numbers for the tray mini-monitor: CPU, memory, disk and network throughput.

use serde::Serialize;
use std::time::Instant;
use sysinfo::{Disks, Networks, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub cpu_percent: f32,
    pub memory_percent: f32,
    /// Read + written per second, all disks.
    pub disk_bytes_per_sec: u64,
    /// Received + sent per second, all adapters.
    pub net_bytes_per_sec: u64,
}

pub struct Sampler {
    sys: System,
    disks: Disks,
    nets: Networks,
    last: Instant,
}

fn err(e: sysinfo::Error) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

impl Sampler {
    pub fn new() -> std::io::Result<Self> {
        let mut sys = System::new().map_err(err)?;
        sys.refresh_cpu_usage();
        Ok(Sampler {
            sys,
            disks: Disks::new_with_refreshed_list().map_err(err)?,
            nets: Networks::new_with_refreshed_list().map_err(err)?,
            last: Instant::now(),
        })
    }

    /// Measures since the previous call (the first one after `new` covers the time since then).
    pub fn sample(&mut self) -> Sample {
        let secs = self.last.elapsed().as_secs_f64().max(0.05);
        self.last = Instant::now();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.disks.refresh(true);
        self.nets.refresh(true);
        let disk: u64 = self
            .disks
            .iter()
            .map(|d| {
                let u = d.usage();
                u.read_bytes + u.written_bytes
            })
            .sum();
        let net: u64 = self
            .nets
            .values()
            .map(|n| n.received() + n.transmitted())
            .sum();
        let total = self.sys.total_memory().max(1);
        Sample {
            cpu_percent: self.sys.global_cpu_usage(),
            memory_percent: (self.sys.used_memory() as f64 / total as f64 * 100.0) as f32,
            disk_bytes_per_sec: (disk as f64 / secs) as u64,
            net_bytes_per_sec: (net as f64 / secs) as u64,
        }
    }
}
