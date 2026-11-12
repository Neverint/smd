//! Live top programs by CPU and memory, grouped by program name like Task Manager.
//! CPU needs two samples: the first call after creating a `Monitor` reports 0 %.

use serde::Serialize;
use std::collections::HashMap;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramUsage {
    pub name: String,
    /// Number of processes with this name.
    pub processes: u32,
    /// Share of the whole CPU (all cores), 0-100.
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub by_cpu: Vec<ProgramUsage>,
    pub by_memory: Vec<ProgramUsage>,
    pub cpu_percent: f32,
    pub process_count: u32,
}

pub struct Monitor {
    sys: System,
    cores: f32,
}

impl Monitor {
    pub fn new() -> std::io::Result<Self> {
        let mut sys = System::new().map_err(|e| std::io::Error::other(format!("{e:?}")))?;
        sys.refresh_cpu_usage();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let cores = std::thread::available_parallelism()
            .map(|n| n.get() as f32)
            .unwrap_or(1.0);
        Ok(Monitor { sys, cores })
    }

    pub fn snapshot(&mut self, top: usize) -> Snapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        let mut groups: HashMap<String, ProgramUsage> = HashMap::new();
        for p in self.sys.processes().values() {
            let name = p.name().to_string_lossy().into_owned();
            if name.is_empty() || name.eq_ignore_ascii_case("System Idle Process") {
                continue;
            }
            let g = groups
                .entry(name.to_lowercase())
                .or_insert_with(|| ProgramUsage {
                    name,
                    processes: 0,
                    cpu_percent: 0.0,
                    memory_bytes: 0,
                });
            g.processes += 1;
            g.cpu_percent += p.cpu_usage() / self.cores;
            g.memory_bytes += p.memory();
        }
        let all: Vec<ProgramUsage> = groups.into_values().collect();
        let mut by_cpu = all.clone();
        by_cpu.sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
        by_cpu.truncate(top);
        let mut by_memory = all;
        by_memory.sort_by_key(|p| std::cmp::Reverse(p.memory_bytes));
        by_memory.truncate(top);
        Snapshot {
            by_cpu,
            by_memory,
            cpu_percent: self.sys.global_cpu_usage(),
            process_count: self.sys.processes().len() as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn groups_live_processes() {
        let mut m = super::Monitor::new().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        let s = m.snapshot(10);
        assert!(s.process_count > 0);
        assert!(!s.by_memory.is_empty() && s.by_memory.len() <= 10);
        assert!(
            s.by_memory
                .windows(2)
                .all(|w| w[0].memory_bytes >= w[1].memory_bytes)
        );
        assert!(
            s.by_cpu
                .iter()
                .all(|p| p.cpu_percent >= 0.0 && p.cpu_percent <= 100.5)
        );
    }
}
