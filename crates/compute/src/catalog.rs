// SPDX-License-Identifier: AGPL-3.0-only
//! Revisioned workload vocabulary from the fixed design inventory.

use serde::{Deserialize, Serialize};

/// Named arithmetic, streaming, matrix or naive LC recipe.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    /// Alternating signed additions.
    Add,
    /// Alternating near-one factors.
    Multiply,
    /// Fused a*b+x with addend dependence.
    FmaAddend,
    /// Fused a*x+b with multiplicand dependence.
    FmaMultiplicand,
    /// Separate multiplication and addition.
    MultiplyAdd,
    /// Alternating runtime division factors.
    Divide,
    /// Square root with timed range-maintenance addition.
    SquareRoot,
    /// Eight-accumulator contiguous reduction.
    Read,
    /// Contiguous scalar fill.
    Write,
    /// Contiguous disjoint copy.
    Copy,
    /// Fused out=a*x+y traversal.
    FusedStream,
    /// Serial raw binary64 matrix product.
    Gemm,
    /// Serial raw binary64 matrix-vector product.
    Gemv,
    /// Serial partial-pivot LU construction.
    Factor,
    /// Serial solve with a prepared partial-pivot LU factor.
    Solve,
    /// Factored-root forward LC phase with fused traversal.
    ForwardFused,
    /// Factored-root forward LC phase with separate rounding.
    ForwardSeparate,
    /// Known-product inductance query.
    Inductance,
    /// Known-product capacitance query.
    Capacitance,
    /// Finite inverse-time branch selection.
    InverseTime,
}

/// Source-level width; no native instruction promise.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Width {
    /// Scalar binary64 source.
    Scalar,
    /// One binary64 SIMD lane.
    Simd1,
    /// Two binary64 SIMD lanes.
    Simd2,
    /// Four binary64 SIMD lanes.
    Simd4,
    /// Eight binary64 SIMD lanes.
    Simd8,
    /// Matrix-owned backend lowering.
    BackendSelectedBinary64,
}

/// Declared dependency and output-consumption structure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Dependency {
    /// One evolving state per lane.
    Dependent,
    /// Eight evolving states per lane.
    Independent,
    /// Eight SIMD partial accumulators.
    MultiAccumulatorReduction,
    /// Independent streaming elements.
    IndependentElements,
    /// Matrix backend operation.
    BackendKernel,
    /// Naive LC output traversal.
    PreparedTraversal,
}

/// Explicit allocation and restoration boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Scope {
    /// Preparation and reset outside the timed operation.
    Prepared,
    /// Named preparation and allocation inside the timer.
    PreparationInclusive,
    /// Repeated factors with timed restoration before each factor.
    RestorationInclusiveRepeated,
}

/// Requested host placement; observed placement is recorded separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Placement {
    /// No requested CPU constraint.
    Unbound,
    /// Caller supplies external CPU-set policy and evidence.
    ExternallyConstrained,
    /// Requests an unavailable safe per-worker binding setter.
    ExactWorkerBinding,
}

/// Versioned inventory descriptor; reading it starts no collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Catalog {
    /// Workload recipe identity.
    pub recipe_revision: &'static str,
    /// Complete named inventory in stable order.
    pub operations: [Operation; 20],
    /// Logical source widths, separate from backend selection.
    pub widths: [Width; 5],
}

/// Return the fixed catalog without sampling or probing the host.
#[must_use]
pub const fn catalog() -> Catalog {
    Catalog {
        recipe_revision: "compute-1",
        operations: [
            Operation::Add,
            Operation::Multiply,
            Operation::FmaAddend,
            Operation::FmaMultiplicand,
            Operation::MultiplyAdd,
            Operation::Divide,
            Operation::SquareRoot,
            Operation::Read,
            Operation::Write,
            Operation::Copy,
            Operation::FusedStream,
            Operation::Gemm,
            Operation::Gemv,
            Operation::Factor,
            Operation::Solve,
            Operation::ForwardFused,
            Operation::ForwardSeparate,
            Operation::Inductance,
            Operation::Capacitance,
            Operation::InverseTime,
        ],
        widths: [
            Width::Scalar,
            Width::Simd1,
            Width::Simd2,
            Width::Simd4,
            Width::Simd8,
        ],
    }
}
