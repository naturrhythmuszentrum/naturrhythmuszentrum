// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit compute instrumentation and exact report quantities.
//!
//! Measurements describe host wall time; they provide no engine correctness evidence.
#![feature(portable_simd)]
#![expect(
    clippy::multiple_crate_versions,
    reason = "Pinned faer/nano-gemm, petgraph/indexmap and gemm-common/sysctl carry duplicate equator, hashbrown and thiserror versions."
)]

pub mod decimal;

mod catalog;
mod identity;
mod request;
pub use catalog::{Catalog, Dependency, Operation, Placement, Scope, Width, catalog};
pub use identity::{Binary64, Seed};
pub use request::{
    CancellationToken, CaseRequest, ExecutionBounds, RequestCause, RequestError, ResourcePolicy,
    RunRequest, SamplingPolicy,
};

pub mod recipe;

mod provenance;
pub use provenance::{
    BuildIntent, BuildRecord, EnvironmentRecord, Evidence, LoweringRecord, PublicProjection,
    Setting,
};

mod adapter;
mod capture;
pub mod codec;
mod collect;
mod kernels;
pub mod layout;
mod load_summary;
mod memory;
pub mod pilot;
pub mod replay;
pub mod report;
pub mod sampling_evidence;
pub mod statistics;
pub mod summary;
pub use codec::{read_report, read_summary, write_report, write_summary};
pub use collect::{RunContext, run};
pub use report::{CaseOutcome, Measurement, PartialMeasurement, RawContentId, RunReport};
pub use summary::{SummaryDataset, SummaryError, summarize};

mod workers;

mod quantities;
pub use quantities::{
    ConfidenceInterval, ElementsPerSecond, Estimate, FloatingOperationsPerSecond, ItemsPerSecond,
    LogicalBytesPerSecond, Rate, SecondsPerItem, VectorOperationsPerSecond,
};

pub mod telemetry;

pub mod admission;

pub use load_summary::{LoadFractions, LoadValue, SummaryLoad, SummaryPressure};
