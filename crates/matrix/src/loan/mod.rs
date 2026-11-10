// SPDX-License-Identifier: AGPL-3.0-only
//! Owned host input follows the matrix loans design without a heap-wide execution borrow.

#[cfg(test)]
mod abi;
mod account;
mod application;
mod declaration;
mod error;
#[cfg(test)]
pub(crate) mod fault;
mod inventory;
mod lifecycle;
mod payload;
pub(crate) mod preparation;
mod reader;
mod registry;
mod retained_expm1;
mod scans;

pub use account::ExecutionAccount;
pub use application::{
    ApplicationCancellation, ApplicationError, ApplicationEvaluation, ApplicationPublication,
    ApplicationRequest, CellOutcome, Expm1ApplicationRequest, OwnedApplication,
    PointwiseExpm1Request, PointwiseRequest, PreparedExpm1Operation,
};
pub use declaration::{HostChannel, HostTileDeclaration};
pub use error::LoanError;
pub use inventory::LoanInventory;
pub use lifecycle::{CancelOutcome, HandbackOutcome, LoanRequest, OwnedLoan};
pub use reader::{BundlePin, PublishedTile, TileReader};
pub use registry::{
    AbsenceReason, CancellationReceipt, LoanTicket, ProducerOutcome, PublicationReceipt,
    QualifiedMaskVersion,
};
pub(crate) use registry::{ManagedDescriptor, Registry};
pub use retained_expm1::{RetainedExpm1, RetainedExpm1Error, RetainedExpm1Inventory};
pub use scans::{Residence, ScanCursor, ScanOutcome};

use nrz_bindings::{Budget, readings::algebra::Location};
/// Native ownership phases share the original ledger's observation boundary.
const FAMILY: &str = "matrix.loan";
/// Stable accounting location independent of optional receipt observation.
const AT: Location = Location::new("native owned host input");
/// Debit exact checked native steps before mutation.
pub(crate) fn charge(budget: &mut Budget, steps: usize) -> Result<(), LoanError> {
    budget.work(
        u64::try_from(steps).map_err(|_| LoanError::CountOverflow)?,
        AT,
    )?;
    Ok(())
}
/// Aligned C-layout mirror of the standard shared-owner accounting boundary.
#[repr(C)]
struct RcLayout<T> {
    /// Strong references.
    strong: usize,
    /// Weak references.
    weak: usize,
    /// Immutable owned payload.
    value: T,
}
/// Count the complete shared allocation, separately from handles in containing records.
pub(crate) const fn rc_bytes<T>() -> usize {
    size_of::<RcLayout<T>>()
}
/// Fixed-capacity native allocation with no later unchecked growth.
fn vector<T>(capacity: usize) -> Result<Vec<T>, LoanError> {
    capacity
        .checked_mul(size_of::<T>())
        .ok_or(LoanError::ByteOverflow)?;
    #[cfg(test)]
    if capacity != 0 {
        fault::before()?;
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| LoanError::AllocationRefused)?;
    if values.capacity() != capacity {
        return Err(LoanError::AllocationRefused);
    }
    Ok(values)
}
/// A packed flag extent does not add unchecked padding.
const fn words(count: usize) -> usize {
    count.div_ceil(64)
}

#[cfg(test)]
mod tests;
