// SPDX-License-Identifier: AGPL-3.0-only
//! Planned, ticketed private buffers for the closed reciprocal kernel schedule.

mod buffer;
mod record;
mod run;

pub(crate) use buffer::Buffer;
pub use record::{PrivateAllocation, PrivateAllocationTicket, ReciprocalInventory};
pub(crate) use run::Run;

/// A private allocation cannot bypass its planned exact meaning or live ticket.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PrivateAllocationError {
    /// A requested exact declaration or stage is absent from this execution's plan.
    #[error("private allocation meaning is not planned")]
    UnplannedMeaning,
    /// A planned buffer can be allocated only once in one candidate attempt.
    #[error("private allocation ticket has already been drawn")]
    DuplicateTicket,
    /// The requested Rust representation disagrees with the planned floating components.
    #[error("private allocation representation mismatch")]
    Representation,
    /// A planned buffer was never allocated or was not completely initialized.
    #[error("private allocation inventory is incomplete")]
    IncompleteCoverage,
    /// A candidate array does not retain the typed permit that authorized its allocation.
    #[error("candidate contains an unticketed private allocation")]
    UnticketedBuffer,
    /// The inventory belongs to another execution, candidate or exact declaration.
    #[error("private allocation inventory belongs to another execution")]
    ForeignInventory,
}
