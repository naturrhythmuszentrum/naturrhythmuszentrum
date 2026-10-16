// SPDX-License-Identifier: AGPL-3.0-only
//! Typed solve refusals preserve arithmetic and physical admission causes.

/// A failed square solve premise, never a proof that the true solution is inaccurate.
#[derive(Clone, Debug, thiserror::Error)]
pub enum SolveError {
    /// Independent outward arithmetic could not produce a finite enclosure.
    #[error("solve arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// An input or computed bound failed numerical admission.
    #[error("solve bound: {0}")]
    Accuracy(#[from] nrz_accuracy::AccuracyFailure),
    /// A checked view or fallible allocation could not be resolved.
    #[error("solve storage: {0}")]
    Heap(#[from] crate::HeapError),
    /// Actual statistical coverage requires the later solve sensitivity law.
    #[error("statistical solve input at slot {slot} is unsupported")]
    StatisticalInput {
        /// Operation-ordered input position.
        slot: usize,
    },
    /// The represented-center subset cannot yet propagate a nonzero input radius.
    #[error("bounded solve inputs require the perturbation theorem")]
    BoundedInputsUnsupported,
    /// Supplied scalar output descriptors disagree with the derived solve axes.
    #[error("solve input or result contract mismatch")]
    Contract,
    /// Backend factors or candidates are nonfinite or have a zero pivot.
    #[error("LU could not produce a finite square solve candidate")]
    CandidateFailure,
    /// The independent inverse defect did not establish a strict Neumann margin.
    #[error("inverse defect did not establish q < 1")]
    CannotCertifyInverse,
    /// A factor belongs to a different validated execution.
    #[error("factor belongs to another execution")]
    ForeignFactor,
    /// Independent contraction work exhausted its checked allowance.
    #[error("solve verification work exhausted")]
    ResourceLimit,
}
