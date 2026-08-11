//! Process discovery helpers built on top of `sysinfo`.
//!
//! The helpers in this module normalize process snapshots into a small
//! application-facing type, making UI components independent of raw crate APIs.

use sysinfo::System;

/// Simplified running-process descriptor for UI selection controls.
#[derive(Debug, Clone)]
pub struct RunningProcess {
    pub pid: u32,
    pub name: String,
}

/// Returns a sorted list of currently running processes.
///
/// Sorting order is name-first and PID-second so users get stable,
/// predictable selector behavior between refreshes.
pub fn list_running_processes(system: &System) -> Vec<RunningProcess> {
    let mut list: Vec<RunningProcess> = system
        .processes()
        .values()
        .map(|p| RunningProcess {
            pid: p.pid().as_u32(),
            name: p.name().to_string_lossy().to_string(),
        })
        .collect();

    list.sort_by(|a, b| a.name.cmp(&b.name).then(a.pid.cmp(&b.pid)));
    list
}
