// SPDX-License-Identifier: AGPL-3.0-only
//! Safe scalar and explicit logical SIMD recipe execution.

mod arithmetic;
mod lc;
mod matrix;
mod stream;

use crate::{
    CaseRequest, Dependency, Seed,
    recipe::{Numerators, RecipeError, arithmetic_counts, stream_counts},
};
use arithmetic::Arithmetic;
use lc::Lc;
use matrix::Matrix;
use stream::Stream;

/// Reconstruct only the selected arithmetic recipe's exact fixed input evidence.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn arithmetic_inputs(
    case: &CaseRequest,
    seed: Seed,
) -> Result<Option<crate::recipe::ArithmeticInputs>, RecipeError> {
    arithmetic::inputs(case, seed)
}

/// Backend capability is resolved without preparing or reading any numerical inputs.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn packing_supported(case: &CaseRequest) -> bool {
    matrix::packing_supported(case)
}

/// Prepared owned workload; operation selection occurs outside timing.
#[derive(Debug)]
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) enum Kernel {
    /// Scalar or SIMD ordinary finite arithmetic.
    Arithmetic(Box<Arithmetic>),
    /// Contiguous streaming buffers.
    Stream(Stream),
    /// Optimized naive LC expressions.
    Lc(Lc),
    /// Serial raw matrix backend.
    Matrix(Box<Matrix>),
}

impl Kernel {
    /// Prepare the explicitly requested family without substituting another case.
    pub(super) fn prepare(
        case: &CaseRequest,
        seed: Seed,
        byte_cap: u64,
    ) -> Result<Self, RecipeError> {
        match case.dependency {
            Dependency::Dependent | Dependency::Independent => {
                let bytes =
                    u64::try_from(size_of::<Arithmetic>()).map_err(|_| RecipeError::Overflow)?;
                if bytes > byte_cap {
                    return Err(RecipeError::Allocation);
                }
                Arithmetic::prepare(case, seed).map(|kernel| Self::Arithmetic(Box::new(kernel)))
            }
            Dependency::MultiAccumulatorReduction | Dependency::IndependentElements => {
                Stream::prepare(case, seed, byte_cap).map(Self::Stream)
            }
            Dependency::PreparedTraversal => Lc::prepare(case, byte_cap).map(Self::Lc),
            Dependency::BackendKernel => {
                Matrix::prepare(case, seed, byte_cap).map(|kernel| Self::Matrix(Box::new(kernel)))
            }
        }
    }
    /// Snapshot worker-owned payload before timers and without allocating metadata.
    pub(super) fn layout(&self, worker: u64) -> Result<crate::layout::WorkerLayout, RecipeError> {
        let buffers = match self {
            Self::Arithmetic(value) => value.layout()?,
            Self::Stream(value) => value.layout()?,
            Self::Lc(value) => value.layout()?,
            Self::Matrix(value) => value.layout()?,
        };
        let total = |capacity| {
            buffers
                .iter()
                .flatten()
                .try_fold(0_u64, |sum, buffer| {
                    sum.checked_add(if capacity {
                        buffer.capacity_bytes
                    } else {
                        buffer.live_bytes
                    })
                })
                .filter(|n| *n <= crate::decimal::MAX_INTEGER)
                .ok_or(RecipeError::Overflow)
        };
        let numerical_capacity_bytes = total(true)?;
        let numerical_live_bytes = total(false)?;
        let (allowed_cpus, allowed_nodes) = crate::layout::allowances();
        Ok(crate::layout::WorkerLayout {
            worker,
            numerical_capacity_bytes,
            numerical_live_bytes,
            first_touch_worker: worker,
            allowed_cpus,
            allowed_nodes,
            startup_cpu: crate::layout::startup_cpu(),
            numa: crate::layout::NumaEvidence::UnavailableSafePageQuery,
            buffers,
        })
    }

    /// Restore mutable outputs outside the timer.
    pub(super) fn reset(&mut self) {
        match self {
            Self::Arithmetic(_) => {}
            Self::Stream(stream) => stream.reset(),
            Self::Lc(lc) => lc.reset(),
            Self::Matrix(matrix) => matrix.reset(),
        }
    }
    /// Complete deterministic finite-domain validation outside measurement.
    pub(super) fn validate(&mut self, length: u64) -> Result<(), RecipeError> {
        match self {
            Self::Arithmetic(arithmetic) => arithmetic.validate(length),
            Self::Stream(stream) => stream.validate(length),
            Self::Lc(lc) => lc.validate(length),
            Self::Matrix(matrix) => matrix.validate(length),
        }
    }
    /// Execute and consume exactly the declared per-worker item.
    pub(super) fn execute(&mut self, length: u64) -> Result<(), RecipeError> {
        match self {
            Self::Arithmetic(arithmetic) => arithmetic.execute(length),
            Self::Stream(stream) => stream.execute(length),
            Self::Lc(lc) => lc.execute(length),
            Self::Matrix(matrix) => matrix.execute(length),
        }
    }
}

/// Derive the selected family's checked aggregate work ledger.
#[expect(
    clippy::redundant_pub_crate,
    reason = "Shared by crate siblings; private module prevents public reachability."
)]
pub(super) fn counts(case: &CaseRequest, length: u64) -> Result<Numerators, RecipeError> {
    match case.dependency {
        Dependency::Dependent | Dependency::Independent => arithmetic_counts(case, length),
        Dependency::MultiAccumulatorReduction | Dependency::IndependentElements => {
            stream_counts(case, length)
        }
        Dependency::PreparedTraversal => lc_counts(case, length),
        Dependency::BackendKernel => matrix::counts(case, length),
    }
}

/// Checked LC work comes from its explicit preparation/traversal ledger.
fn lc_counts(case: &CaseRequest, length: u64) -> Result<Numerators, RecipeError> {
    crate::recipe::LcLedger::from_case(case)?.numerators(case.scope, length)
}
