// SPDX-License-Identifier: AGPL-3.0-only
//! Tagged captured facts and separately marked caller declarations.

use serde::{Deserialize, Serialize};

/// Missing evidence never silently becomes a known or default setting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "value", deny_unknown_fields)]
pub enum Evidence<T> {
    /// Captured value with its method retained by the containing field.
    Known(T),
    /// A fact could not be established, with a bounded reason.
    Unknown(String),
    /// Probe missing or denied, with its explicit reason.
    Unavailable(String),
    /// Explicit public projection, never a reversible private mapping.
    Redacted(String),
}

/// Compile-time target selection intent, distinct from observed machine-code lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum BuildIntent {
    /// No native CPU or explicit target-feature selection was requested.
    Baseline,
    /// The effective target CPU option requests the build machine's native CPU.
    NativeRequested,
    /// An explicit CPU or target-feature set was requested.
    ExplicitTarget,
}

/// Structured machine-code inspection evidence; build intent never substitutes for inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoweringRecord {
    /// Whether an explicit inspection occurred.
    pub inspected: Evidence<bool>,
    /// Full content identity of the inspected executable or object.
    pub artifact: Evidence<crate::report::RawContentId>,
    /// Exact inspected symbol names.
    pub symbols: Evidence<Vec<String>>,
    /// Evidence of scalar-source auto-vectorization.
    pub scalar_vectorization: Evidence<String>,
    /// Evidence of splitting a logical vector width.
    pub splitting: Evidence<String>,
    /// Evidence of out-of-line helpers.
    pub helpers: Evidence<String>,
    /// Whether inspected FMA operations use native instructions.
    pub native_fma: Evidence<bool>,
    /// Bounded inspection method and evidence notes.
    pub notes: Evidence<String>,
}

/// Build facts captured at compile time; runtime callers cannot replace them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRecord {
    /// Full compiler verbose version.
    pub compiler: Evidence<String>,
    /// Source HEAD at build time.
    pub source_commit: Evidence<String>,
    /// Whether tracked or untracked source was dirty at capture.
    pub dirty: Evidence<bool>,
    /// Algorithm-qualified lockfile blob identity from checkout root.
    pub lockfile: Evidence<String>,
    /// Cargo target triple.
    pub target: Evidence<String>,
    /// Cargo profile.
    pub profile: Evidence<String>,
    /// Effective inherited Cargo encoded rustflags.
    pub flags: Evidence<String>,
    /// Captured target selection intent; runtime ISA never upgrades a baseline build.
    pub intent: Evidence<BuildIntent>,
    /// Enabled Cargo features; distinct from ISA features.
    pub features: Vec<String>,
    /// Backend package/version and explicit serial execution policy.
    pub backend: Evidence<String>,
    /// Relevant package versions captured from the resolved checkout lockfile.
    pub dependencies: Evidence<Vec<String>>,
    /// Inspection of actual lowered instructions, absent without explicit evidence.
    pub lowering: LoweringRecord,
}

/// Explicit control request and observed state, without conflating them.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setting {
    /// Caller control declaration, not a verified probe.
    pub requested: Evidence<String>,
    /// Observed setting or explicit unavailable state.
    pub observed: Evidence<String>,
    /// Capture method, not executable instructions.
    pub method: String,
    /// Capture offset in integer nanoseconds from run entry.
    pub offset_ns: u64,
}

/// Bounded public machine environment; numeric telemetry uses typed source fields.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentRecord {
    /// Opaque caller-assigned alias, not a hostname.
    pub machine_alias: String,
    /// Public CPU model identity.
    pub cpu_model: Evidence<String>,
    /// Runtime detected ISA names from a fixed revisioned list.
    pub runtime_isa: Vec<String>,
    /// Compile-time ISA names from the same list.
    pub compile_isa: Vec<String>,
    /// Topology policy and observed state.
    pub topology: Setting,
    /// Private/shared cache configuration.
    pub caches: Setting,
    /// Memory capacity/configuration.
    pub memory: Setting,
    /// SMT state.
    pub smt: Setting,
    /// Worker and coordinator placement.
    pub placement: Setting,
    /// NUMA policy and observed state.
    pub numa: Setting,
    /// Public operating-system/kernel version.
    pub kernel: Setting,
    /// Distribution release fields, captured independently of the kernel version.
    pub distribution: Setting,
    /// Container/VM limits with identifiers removed in public projection.
    pub resource_limits: Setting,
    /// Frequency governor/power policy.
    pub power: Setting,
    /// Turbo state.
    pub turbo: Setting,
    /// Scoped tracing declaration; no subscriber is installed.
    pub tracing: Setting,
    /// Concurrent workload declaration, explicitly unverified.
    pub concurrent_load: Setting,
    /// Public core-class inventory; unavailable classes are not guessed.
    pub core_classes: Setting,
    /// Observed coordinator CPU allowance, distinct from binding.
    pub coordinator_placement: Setting,
    /// Observed worker placement, distinct from requested process constraints.
    pub worker_placement: Setting,
    /// Observed process NUMA-node allowance, not page residency.
    pub numa_allowed_nodes: Setting,
    /// Process resource limits without process identifiers.
    pub process_limits: Setting,
    /// Resolved unified-cgroup CPU quota and period.
    pub cgroup_cpu_limit: Setting,
    /// Resolved unified-cgroup memory limit.
    pub cgroup_memory_limit: Setting,
    /// Resolved effective unified-cgroup CPU allowance.
    pub cgroup_cpuset: Setting,
    /// Virtualization evidence without host or container identifiers.
    pub virtual_machine: Setting,
    /// Observed CPU-zero minimum policy frequency in native kHz.
    pub frequency_min: Setting,
    /// Observed CPU-zero maximum policy frequency in native kHz.
    pub frequency_max: Setting,
}

/// Explicit deterministic public projection retained in the externally held raw report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicProjection {
    /// Public build fields after recorded redaction.
    pub build: BuildRecord,
    /// Public environment fields after recorded redaction.
    pub environment: EnvironmentRecord,
    /// Executed argument vector with private paths explicitly replaced.
    pub command: Vec<String>,
    /// Non-reversible descriptions of each public-field transformation.
    pub redactions: Vec<String>,
}
