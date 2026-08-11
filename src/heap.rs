//! Heap inspection abstraction layer and placeholder backend.
//!
//! This module intentionally defines a stable contract for future
//! platform-specific heap inspection implementations while keeping current
//! runtime behavior explicit (`not implemented`).
//!
//! Benefits of this shape:
//! - UI and orchestration code can target a trait today.
//! - Backends can be introduced incrementally per platform/privilege model.
//! - Error semantics are consistent from day one.

use anyhow::{Result, bail};

/// Requested granularity for heap inspection snapshots.
#[derive(Debug, Clone, Copy)]
pub enum HeapDetailLevel {
    Summary,
    AllocationSites,
    Full,
}

/// Input payload for a single heap inspection request.
#[derive(Debug, Clone)]
pub struct HeapInspectionRequest {
    pub pid: u32,
    pub detail_level: HeapDetailLevel,
}

/// Summary result payload produced by heap backends.
#[derive(Debug, Clone, Default)]
pub struct HeapSnapshotSummary {
    pub inspected_pid: u32,
    pub total_heap_mib: f64,
    pub allocation_count: u64,
    pub timestamp_unix_sec: u64,
}

/// Backend contract for platform-specific heap inspection providers.
pub trait HeapInspectorBackend {
    /// Indicates whether this backend is operational in the current environment.
    ///
    /// Implementations may return `false` if unsupported platform APIs,
    /// insufficient permissions, or missing capabilities are detected.
    fn is_supported(&self) -> bool;

    /// Performs a heap snapshot request and returns normalized summary data.
    ///
    /// Backends are expected to return rich errors for unsupported requests,
    /// permission failures, or transient inspection faults.
    fn inspect_heap(&self, request: &HeapInspectionRequest) -> Result<HeapSnapshotSummary>;
}

/// Stub backend used until real heap inspection is implemented.
#[derive(Default)]
pub struct StubHeapInspector;

impl HeapInspectorBackend for StubHeapInspector {
    /// Reports that stub backend is intentionally unavailable.
    fn is_supported(&self) -> bool {
        false
    }

    /// Returns a deterministic `not implemented` error for all requests.
    fn inspect_heap(&self, request: &HeapInspectionRequest) -> Result<HeapSnapshotSummary> {
        let _ = request;
        bail!("Heap-Inspection ist vorbereitet, aber noch nicht implementiert")
    }
}
