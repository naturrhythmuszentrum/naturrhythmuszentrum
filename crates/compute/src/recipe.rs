// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic finite input patterns and explicit work numerators.

mod lc;
pub use lc::{LcLedger, OperationCounts};

use crate::decimal::MAX_INTEGER;
use crate::{CaseRequest, Dependency, Operation, Seed, Width};

mod protocol;
pub use protocol::{
    AllocationStrategy, CachePolicy, CaseProtocol, ConditionEstimate, Conditioning,
    MatrixConstruction, ReadOrder, ResetPolicy, RestorationSchedule, StartWaitPolicy,
};

/// Public packing capability evidence, independent of unavailable backend tuning thresholds.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackingEvidence {
    /// Source-selected allocation-free path, or why no bounded path is available.
    pub path: crate::Evidence<String>,
    /// Declared internal packing bytes; zero only on proven bypass paths.
    pub bytes: crate::Evidence<u64>,
    /// RHS packing threshold is inaccessible through the pinned public dependency surface.
    pub rhs_threshold: crate::Evidence<u64>,
    /// Single-thread LHS threshold has the same unavailable public route.
    pub lhs_single_thread_threshold: crate::Evidence<u64>,
}

impl PackingEvidence {
    /// Record a source-proven bypass without inferring any hidden threshold value.
    pub(crate) fn from_case(case: &CaseRequest) -> Result<Self, RecipeError> {
        // Four strings, each bounded by the report's named packing-text reservation.
        Self::from_case_owned(case, &mut crate::memory::Budget::new(4 * 65_537))
    }

    /// Retain packing reasons only after their capacities fit the shared report allowance.
    pub(crate) fn from_case_owned(
        case: &CaseRequest,
        budget: &mut crate::memory::Budget,
    ) -> Result<Self, RecipeError> {
        let supported = crate::kernels::packing_supported(case);
        let path = if supported {
            let name = match case.operation {
                Operation::Factor => "order-one LU",
                Operation::Solve => "order-one solve",
                Operation::Gemv => "contiguous matrix-vector dispatch",
                _ => {
                    if case.shape.first() == Some(&1) || case.shape.last() == Some(&1) {
                        "contiguous matrix-vector dispatch"
                    } else {
                        "contiguous rank-one dispatch"
                    }
                }
            };
            crate::Evidence::Known(budget.text(name)?)
        } else {
            crate::Evidence::Unavailable(
                budget.text("opaque backend packing has no public requirement")?,
            )
        };
        let bytes = if supported {
            crate::Evidence::Known(0)
        } else {
            crate::Evidence::Unavailable(budget.text("no public packing requirement")?)
        };
        let mut threshold = || -> Result<_, RecipeError> {
            Ok(crate::Evidence::Unavailable(budget.text("pinned faer does not re-export GEMM threshold getters; no new dependency is admitted")?))
        };
        Ok(Self {
            path,
            bytes,
            rhs_threshold: threshold()?,
            lhs_single_thread_threshold: threshold()?,
        })
    }
}

/// Checked named work per aggregate item; absent quantities have no zero rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Numerators {
    /// Exactly one aggregate item per timed invocation.
    pub items: u64,
    /// Scalar output/update elements.
    pub elements: u64,
    /// Logical vector operations, absent for scalar/backend sources.
    pub vector_operations: Option<u64>,
    /// Conventional aggregate arithmetic operations, when the recipe defines them.
    pub floating_operations: Option<u64>,
    /// Logical operand traffic, never physical bus bytes.
    pub logical_bytes: Option<u64>,
}

/// Invalid recipe domain or checked count overflow.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize, thiserror::Error,
)]
pub enum RecipeError {
    /// Invalid finite input or recipe combination.
    #[error("invalid recipe domain")]
    Domain,
    /// Recipe count or capacity overflow.
    #[error("recipe count overflow")]
    Overflow,
    /// Requested preparation allocation failed.
    #[error("recipe allocation failed")]
    Allocation,
    /// A worker channel disconnected or returned a mismatched generation.
    #[error("worker protocol failed")]
    Worker,
    /// Cooperative cancellation was observed.
    #[error("recipe cancelled")]
    Cancelled,
}

/// Reproducible exact binary64 input from the seed's effective 20-bit offset.
/// # Errors
/// Refuses an impossible conversion outside the fixed 20-bit generator range.
pub fn input(seed: Seed, position: u64) -> Result<f64, RecipeError> {
    let offset = u64::try_from(seed.offset()).map_err(|_| RecipeError::Overflow)?;
    let index = (offset + 8191 * (position % 1_048_576)) % 1_048_576;
    let index = u32::try_from(index).map_err(|_| RecipeError::Overflow)?;
    Ok(1.0 + f64::from(index) / 1_048_576.0)
}

/// Lane count for explicitly specialized source widths.
pub(crate) const fn lanes(width: Width) -> u64 {
    match width {
        Width::Scalar | Width::Simd1 | Width::BackendSelectedBinary64 => 1,
        Width::Simd2 => 2,
        Width::Simd4 => 4,
        Width::Simd8 => 8,
    }
}

/// Product with the shared wire-bound refusal.
fn product(values: &[u64]) -> Result<u64, RecipeError> {
    values
        .iter()
        .try_fold(1_u64, |p, n| {
            p.checked_mul(*n).filter(|v| *v <= MAX_INTEGER)
        })
        .ok_or(RecipeError::Overflow)
}

/// Derive arithmetic numerators independently of observed elapsed time.
/// # Errors
/// Rejects non-arithmetic operations and count overflow.
pub fn arithmetic_counts(case: &CaseRequest, length: u64) -> Result<Numerators, RecipeError> {
    let states = match case.dependency {
        Dependency::Dependent => 1,
        Dependency::Independent => 8,
        Dependency::MultiAccumulatorReduction
        | Dependency::IndependentElements
        | Dependency::BackendKernel
        | Dependency::PreparedTraversal => return Err(RecipeError::Domain),
    };
    let factor = match case.operation {
        Operation::Add | Operation::Multiply | Operation::Divide => 1,
        Operation::FmaAddend
        | Operation::FmaMultiplicand
        | Operation::MultiplyAdd
        | Operation::SquareRoot => 2,
        Operation::Read
        | Operation::Write
        | Operation::Copy
        | Operation::FusedStream
        | Operation::Gemm
        | Operation::Gemv
        | Operation::Factor
        | Operation::Solve
        | Operation::ForwardFused
        | Operation::ForwardSeparate
        | Operation::Inductance
        | Operation::Capacitance
        | Operation::InverseTime => return Err(RecipeError::Domain),
    };
    let operations = product(&[length, states, case.workers])?;
    let elements = product(&[operations, lanes(case.width)])?;
    Ok(Numerators {
        items: 1,
        elements,
        vector_operations: if case.width == Width::Scalar {
            None
        } else {
            Some(operations)
        },
        floating_operations: Some(product(&[elements, factor])?),
        logical_bytes: None,
    })
}

/// Explicit streaming byte/add ledger, including the reduction's combining adds.
/// # Errors
/// Rejects invalid working sets or checked numerator overflow.
pub fn stream_counts(case: &CaseRequest, passes: u64) -> Result<Numerators, RecipeError> {
    let n = *case.shape.first().ok_or(RecipeError::Domain)?;
    let population = product(&[passes, case.workers])?;
    let elements = product(&[population, n])?;
    let (bytes, floating, vectors) = match case.operation {
        Operation::Read => {
            let width = lanes(case.width);
            let accumulators = product(&[8, width])?;
            if n < accumulators {
                return Err(RecipeError::Domain);
            }
            let combining = accumulators - 1;
            let adds = n.checked_add(combining).ok_or(RecipeError::Overflow)?;
            (
                8,
                Some(product(&[population, adds])?),
                Some(product(&[population, n / accumulators, 8])?),
            )
        }
        Operation::Write => (8, None, None),
        Operation::Copy => (16, None, None),
        Operation::FusedStream => (24, Some(product(&[elements, 2])?), None),
        _ => return Err(RecipeError::Domain),
    };
    Ok(Numerators {
        items: 1,
        elements,
        vector_operations: vectors,
        floating_operations: floating,
        logical_bytes: Some(product(&[elements, bytes])?),
    })
}

/// Exact active arithmetic inputs in accumulator-major, lane-minor order.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArithmeticInputs {
    /// Active initial lane states, with no inactive padding lanes.
    pub states: Vec<crate::Binary64>,
    /// Alternating coefficient tuples; tuple fields are adjacent for each lane.
    pub pair: [Vec<crate::Binary64>; 2],
    /// Number of coefficient fields per tuple, one or two.
    pub tuple_fields: u64,
}
