// SPDX-License-Identifier: AGPL-3.0-only
//! Closed refusals preserve the owning capability in consuming outcomes.

/// A refused host operation; ownership is retained by its operation's outcome.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LoanError {
    /// Nonwrapping scan revisions preserve every pending terminal entitlement.
    #[error("registry revision capacity exhausted")]
    RevisionCapacity,
    /// A continuation was supplied a different execution ledger.
    #[error("foreign execution account")]
    ForeignAccount,
    /// Heap scope is checked before indexing any occupation.
    #[error("foreign numerical owner")]
    ForeignOwner,
    /// The exact occupation has retired or been reused.
    #[error("stale occupation")]
    StaleOccupation,
    /// This producer has already completed in its current occupation.
    #[error("duplicate return")]
    DuplicateReturn,
    /// Request, nonce, reservation or liveness authority differs.
    #[error("wrong loan authority")]
    WrongLoan,
    /// Component layout or axis association differs.
    #[error("layout mismatch")]
    LayoutMismatch,
    /// Binary incidence admits equal extents or a right tile scalar only.
    #[error("unsupported binary operand extent")]
    UnsupportedBinaryExtent,
    /// A selected cell lacks initialized complete coverage.
    #[error("unfilled selected cell")]
    CoverageMismatch,
    /// A companion names a different target.
    #[error("companion target mismatch")]
    TargetMismatch,
    /// The declaration is outside the closed independent host adapter.
    #[error("unsupported host contract")]
    UnsupportedContract,
    /// A selected initialized component fails its declared numerical validator.
    #[error("invalid payload in {channel:?} at cell {cell}, component {component}")]
    InvalidPayload {
        /// Failed channel.
        channel: super::HostChannel,
        /// Logical cell.
        cell: usize,
        /// Component within that cell.
        component: usize,
    },
    /// A current dependency no longer has its admitted version.
    #[error("stale dependency")]
    StaleDependency,
    /// Native count arithmetic cannot be represented.
    #[error("native count overflow")]
    CountOverflow,
    /// Byte arithmetic cannot be represented.
    #[error("native byte overflow")]
    ByteOverflow,
    /// A fallible exact-capacity allocation refused.
    #[error("allocation refused")]
    AllocationRefused,
    /// Existing heap identity, capacity or layout validation refused.
    #[error("loan heap: {0}")]
    Heap(#[from] crate::HeapError),
    /// Existing mask validation refused.
    #[error("loan mask: {0}")]
    Mask(#[from] crate::MaskError),
    /// The original ledger refused Work or a byte ceiling.
    #[error("loan accounting: {0}")]
    Budget(#[from] nrz_bindings::readings::algebra::Fault),
}
