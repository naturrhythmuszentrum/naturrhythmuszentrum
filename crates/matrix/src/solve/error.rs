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
    /// Mixed deterministic scopes cannot be silently promoted to declared-input coverage.
    #[error("solve inputs require compatible deterministic coverage")]
    CoverageMismatch,
    /// Declared physical coverage lacks its complete componentwise total.
    #[error("solve input {slot} lacks declared deterministic coverage")]
    MissingCoverage {
        /// Operation-ordered input position with missing coverage.
        slot: usize,
    },
    /// The common operator family has no established strict invertibility margin.
    #[error("solve perturbation margin does not establish g r_A < 1")]
    PerturbationMargin,
    /// Budget coordinates differ from the restored output difference coordinates.
    #[error("solve budget kind, role or coordinates do not match the output")]
    BudgetContract,
    /// The available certified bound exceeds the requested threshold, not necessarily the actual error.
    #[error("available solve certificate exceeds the requested budget")]
    BudgetExceeded,
    /// The certified upper radius overlaps the tolerance enclosure's acceptance boundary.
    #[error("solve budget comparison is unresolved")]
    BudgetUnresolved,
    /// Supplied scalar output descriptors disagree with the derived solve axes.
    #[error("solve input or result contract mismatch")]
    Contract,
    /// Backend factors or candidates are nonfinite or have a zero pivot.
    #[error("LU could not produce a finite square solve candidate")]
    CandidateFailure,
    /// The independent inverse defect did not establish a strict Neumann margin.
    #[error("inverse defect did not establish q < 1")]
    CannotCertifyInverse,
    /// A factor belongs to a different operator version, selection or checked recipe revision.
    #[error("factor operator dependency key does not match this execution")]
    ForeignFactor,
    /// The authorized correction count ended without meeting the requested certificate budget.
    #[error("solve refinement limit reached without meeting the budget")]
    RefinementLimit,
    /// No smaller certified output radius appeared within the authorized stagnation window.
    #[error("solve certified radius stagnated")]
    Stagnation,
    /// Independent contraction work exhausted its checked allowance.
    #[error("solve verification work exhausted")]
    ResourceLimit,
}
