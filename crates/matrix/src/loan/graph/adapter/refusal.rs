// SPDX-License-Identifier: AGPL-3.0-only
//! Retry classes preserve the native error and never invent a byte decomposition.

use super::native::Failure;
use crate::{
    ApplicationError, HeapError, LcProductError, LcShapeError, LoanError, MaskError, OperationError,
};
use nrz_bindings::{
    AdmissionError, UnitError,
    readings::algebra::{Fault, FaultKind, limits::BudgetKind},
};

/// A byte preflight retains its own dimension, baseline and incremental peak.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bytes {
    /// Retained versus staged byte dimension from the exact phase preflight.
    pub(super) staged: bool,
    /// Releasable baseline included by that same scope.
    pub(super) baseline: u64,
    /// Complete addition, including mandatory simultaneous replacement storage.
    pub(super) addition: u64,
    /// Original immutable ceiling for this dimension.
    pub(super) limit: u64,
}

/// Closed scheduling status does not replace the original structured cause.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Class {
    /// Original cumulative Work stops the whole owner.
    WorkStopped,
    /// A proven fitting addition is blocked by a releasable baseline.
    BytesWait {
        /// Exact incremental requested addition in the refusing byte dimension.
        required: u64,
        /// Remaining capacity after that same scope's releasable baseline.
        available: u64,
    },
    /// Every precision's addition fits its original ceiling with representable aggregate arithmetic.
    ElementsWait,
    /// A fallible allocation refused after native preflight.
    AllocationWait,
    /// Existing outstanding ownership must be resolved independently.
    OwnershipWait,
    /// Only explicit original-closure repair or cancellation may continue.
    Repair,
    /// No temporary capacity interpretation is established.
    Terminal,
}

/// Match the returned whole-scope quantity before applying the adopted B + A rule.
const fn bytes(used: u64, limit: u64, staged: bool, scope: Option<Bytes>) -> Class {
    let Some(scope) = scope else {
        return Class::Terminal;
    };
    let Some(total) = scope.baseline.checked_add(scope.addition) else {
        return Class::Terminal;
    };
    let matching = scope.staged == staged && scope.limit == limit;
    let exact = total == used;
    if matching && exact && scope.addition <= limit {
        Class::BytesWait {
            required: scope.addition,
            available: limit.saturating_sub(scope.baseline),
        }
    } else {
        Class::Terminal
    }
}

/// The native per-call layout `WorkLimit` is deliberately absent from cumulative Work stops.
pub(super) const fn loan(error: &LoanError, scope: Option<Bytes>) -> Class {
    match error {
        LoanError::Budget(error) | LoanError::Mask(MaskError::Budget(error)) => fault(error, scope),
        LoanError::Heap(error) | LoanError::Mask(MaskError::Heap(error)) => heap(error, scope),
        LoanError::AllocationRefused => Class::AllocationWait,
        LoanError::StaleOccupation | LoanError::StaleDependency => Class::Repair,
        _ => Class::Terminal,
    }
}

/// Native arithmetic and invocation dimensions never become byte-capacity retries.
const fn fault(error: &Fault, scope: Option<Bytes>) -> Class {
    match error.kind {
        FaultKind::Exhausted {
            budget: BudgetKind::Work,
            ..
        } => Class::WorkStopped,
        FaultKind::Exhausted {
            budget: BudgetKind::PayloadBytes,
            needed,
            limit,
            ..
        } => bytes(needed, limit, false, scope),
        FaultKind::Exhausted {
            budget: BudgetKind::StagedPayloadBytes,
            needed,
            limit,
            ..
        } => bytes(needed, limit, true, scope),
        _ => Class::Terminal,
    }
}

/// An unclassified aggregate element refusal is terminal; unrelated residents are not subtracted.
const fn heap(error: &HeapError, scope: Option<Bytes>) -> Class {
    match error {
        HeapError::ResourceBytes {
            staged,
            used,
            limit,
        } => bytes(*used, *limit, *staged, scope),
        HeapError::OwnershipPending | HeapError::PinnedUses => Class::OwnershipWait,
        HeapError::StaleHandle
        | HeapError::StaleLayout
        | HeapError::StaleEvidence
        | HeapError::VersionMismatch { .. } => Class::Repair,
        HeapError::Symbols(error) => admission(error, scope),
        HeapError::Units(UnitError::Arithmetic(error)) => fault(error, scope),
        _ => Class::Terminal,
    }
}

/// Original exact Work may be wrapped by unit admission without changing its ledger identity.
const fn admission(error: &AdmissionError, scope: Option<Bytes>) -> Class {
    match error {
        AdmissionError::Algebra(error) | AdmissionError::Units(UnitError::Arithmetic(error)) => {
            fault(error, scope)
        }
        _ => Class::Terminal,
    }
}

/// Unwrap the closed native carriers while leaving the full error available to its caller.
pub(super) const fn classify(error: &Failure, scope: Option<Bytes>) -> Class {
    match error {
        Failure::Native(error)
        | Failure::Execution(LcProductError::Loan(error))
        | Failure::Mapped(ApplicationError::Loan(error)) => loan(error, scope),
        Failure::Preparation(error) => preparation(error, scope),
        Failure::Mapped(ApplicationError::Operation(error)) => operation(error, scope),
        Failure::Mapped(
            ApplicationError::UnsupportedBinaryStructure { .. }
            | ApplicationError::UnsupportedStructure { .. },
        )
        | Failure::Execution(LcProductError::Kernel(_)) => Class::Terminal,
    }
}

/// Consuming preparation retains its full original error outside the graph.
pub(super) const fn preparation(error: &LcShapeError, scope: Option<Bytes>) -> Class {
    match error {
        LcShapeError::Loan(error) => loan(error, scope),
        LcShapeError::Declaration(error) => admission(error, scope),
    }
}

/// Checked mapped-copy operations preserve nested heap and exact accounting causes.
const fn operation(error: &OperationError, scope: Option<Bytes>) -> Class {
    use nrz_bindings::readings::ReadingError;
    match error {
        OperationError::Heap(error) => heap(error, scope),
        OperationError::Conversion(crate::conversion::ConversionError::Map(error))
        | OperationError::Reading(ReadingError::Map(error)) => map(error, scope),
        OperationError::Shape(
            crate::ShapeError::Units(UnitError::Arithmetic(error))
            | crate::ShapeError::Pairing(nrz_bindings::PairingError::Units(UnitError::Arithmetic(
                error,
            ))),
        )
        | OperationError::Exact(error)
        | OperationError::Reading(
            ReadingError::Algebra(error) | ReadingError::Units(UnitError::Arithmetic(error)),
        ) => fault(error, scope),
        _ => Class::Terminal,
    }
}

/// Only the exact capacity carrier can consume precision-specific preflight evidence.
pub(super) const fn loan_capacity(error: &LoanError) -> bool {
    matches!(
        error,
        LoanError::Heap(HeapError::Capacity)
            | LoanError::Mask(MaskError::Heap(HeapError::Capacity))
    )
}

/// Nested native errors retain the same preflight evidence through their owning interfaces.
pub(super) const fn capacity(error: &Failure) -> bool {
    match error {
        Failure::Native(error)
        | Failure::Execution(LcProductError::Loan(error))
        | Failure::Mapped(ApplicationError::Loan(error)) => loan_capacity(error),
        Failure::Mapped(ApplicationError::Operation(OperationError::Heap(HeapError::Capacity))) => {
            true
        }
        _ => false,
    }
}

/// Exact map and formal-pairing wrappers preserve the original arithmetic ledger dimension.
const fn map(error: &nrz_bindings::MapError, scope: Option<Bytes>) -> Class {
    match error {
        nrz_bindings::MapError::Algebra(error)
        | nrz_bindings::MapError::Units(UnitError::Arithmetic(error))
        | nrz_bindings::MapError::Pairing(nrz_bindings::PairingError::Units(
            UnitError::Arithmetic(error),
        )) => fault(error, scope),
        _ => Class::Terminal,
    }
}
