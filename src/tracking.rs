//! Live RAM tracking engine and history retention.
//!
//! This module maintains time-series samples per process group and exposes
//! chart-ready iterators for UI rendering.
//!
//! Sampling model:
//! - each profile group is aggregated into one memory value (MiB),
//! - fixed-size histories are maintained with bounded memory usage,
//! - missing PID targets naturally resolve to `0.0` for that sample.

use std::{collections::HashMap, collections::VecDeque, time::Instant};

use sysinfo::{ProcessesToUpdate, System};

use crate::config::{ProcessTarget, Profile};

const MAX_SAMPLES: usize = 600;

/// One RAM sample point in chart space.
#[derive(Debug, Clone, Copy)]
pub struct SamplePoint {
    pub t_sec: f64,
    pub value_mib: f64,
}

/// In-memory RAM tracker keyed by group name.
#[derive(Default)]
pub struct RamTracker {
    started_at: Option<Instant>,
    group_history: HashMap<String, VecDeque<SamplePoint>>,
}

impl RamTracker {
    /// Creates an empty tracker with no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all recorded group history and restarts relative time origin.
    pub fn clear(&mut self) {
        self.group_history.clear();
        self.started_at = Some(Instant::now());
    }

    /// Samples all groups in the provided profile and appends chart points.
    ///
    /// The function refreshes process data, computes one aggregated memory
    /// value per group, truncates history to `MAX_SAMPLES`, and removes stale
    /// group histories that are no longer present in the profile.
    pub fn sample_profile(&mut self, profile: &Profile, system: &mut System) {
        system.refresh_processes(ProcessesToUpdate::All, true);

        let started_at = self.started_at.get_or_insert_with(Instant::now);
        let t_sec = started_at.elapsed().as_secs_f64();

        for group in &profile.groups {
            let group_mib = group
                .targets
                .iter()
                .map(|target| target_memory_mib(target, system))
                .sum::<f64>();

            let series = self.group_history.entry(group.name.clone()).or_default();
            series.push_back(SamplePoint {
                t_sec,
                value_mib: group_mib,
            });

            while series.len() > MAX_SAMPLES {
                series.pop_front();
            }
        }

        self.group_history.retain(|group_name, _| {
            profile
                .groups
                .iter()
                .any(|group| group.name.eq(group_name))
        });
    }

    /// Returns an iterator over all group history series.
    ///
    /// The iterator is borrowed from internal storage and therefore cheap
    /// to create during each render pass.
    pub fn group_series(&self) -> impl Iterator<Item = (&str, &VecDeque<SamplePoint>)> {
        self.group_history
            .iter()
            .map(|(name, series)| (name.as_str(), series))
    }
}

/// Resolves one target into a memory value in MiB.
///
/// Resolution strategy:
/// - PID targets use exact match and return `0.0` if the PID no longer exists.
/// - Name targets aggregate all process instances with exact lowercase equality.
fn target_memory_mib(target: &ProcessTarget, system: &System) -> f64 {
    if let Some(target_pid) = target.pid {
        if let Some(process) = system
            .processes()
            .values()
            .find(|proc_info| proc_info.pid().as_u32() == target_pid)
        {
            return bytes_to_mib(process.memory());
        }
        return 0.0;
    }

    let target_name = target.process_name.trim().to_lowercase();
    if target_name.is_empty() {
        return 0.0;
    }

    system
        .processes()
        .values()
        .filter(|process| process.name().to_string_lossy().to_lowercase() == target_name)
        .map(|process| bytes_to_mib(process.memory()))
        .sum()
}

/// Converts bytes to mebibytes.
fn bytes_to_mib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}
