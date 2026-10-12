// SPDX-License-Identifier: AGPL-3.0-only
//! Typed refusals shared by the small prototype boundaries.

use std::sync::atomic::{AtomicU64, Ordering};

/// Prototype admission failures; no variant denotes numerical certification.
#[derive(Debug)]
pub enum Error {
    /// An exact units operation failed.
    Units(nrz_units::UnitError),
    /// Exact coefficient arithmetic failed.
    Arithmetic(nrz_algebra::ArithmeticError),
    /// Expression construction or collection failed.
    Algebra(nrz_algebra::Error),
    /// Original source parsing failed.
    Parse(nrz_algebra::ParseError),
    /// Original checking failed with client and CAS locations preserved.
    Check {
        /// Client operation number.
        node: usize,
        /// Client slot, if applicable.
        slot: Option<usize>,
        /// Actual storage reference, when checking a concrete binding.
        reference: Option<crate::ReferenceInfo>,
        /// Binding identity, when checking binding or publication.
        binding: Option<u64>,
        /// Full CAS path, span, definitions, expected and derived evidence.
        source: Box<nrz_units::Diagnostic>,
    },
    /// A schema belongs to another pool or does not exist.
    Schema,
    /// Extents, axes, entry map or a declared shape disagree.
    Shape,
    /// A reference belongs to another heap or has expired.
    Reference,
    /// A value version no longer matches retained evidence.
    Version,
    /// No further unique identities or generations are available.
    Exhausted,
    /// A nonfinite coefficient or singular toy system was refused.
    Numerical,
    /// A conversion, normalization map or candidate identity disagrees.
    Map,
    /// The requested operation is outside the explicitly supported scope.
    Unsupported,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "descriptor prototype refusal: {self:?}")
    }
}

impl std::error::Error for Error {}

impl From<nrz_units::UnitError> for Error {
    fn from(value: nrz_units::UnitError) -> Self {
        Self::Units(value)
    }
}

impl From<nrz_algebra::ArithmeticError> for Error {
    fn from(value: nrz_algebra::ArithmeticError) -> Self {
        Self::Arithmetic(value)
    }
}

impl From<nrz_algebra::Error> for Error {
    fn from(value: nrz_algebra::Error) -> Self {
        Self::Algebra(value)
    }
}

impl From<nrz_algebra::ParseError> for Error {
    fn from(value: nrz_algebra::ParseError) -> Self {
        Self::Parse(value)
    }
}

/// Process-local identities never wrap or compare as unscoped indices.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a pool, heap, snapshot or binding identity without wrapping.
#[expect(
    unreachable_pub,
    reason = "identity allocation stays inside the private prototype module"
)]
pub fn identity() -> Result<u64, Error> {
    NEXT_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| Error::Exhausted)
}
