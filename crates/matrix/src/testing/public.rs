// SPDX-License-Identifier: AGPL-3.0-only
//! Public fixtures only; no crate-private heap state is available here.

use nrz_scalar::Scalar;
use nrz_units::{Descriptor, Dimension, Kind, Measure, Role};

use nrz_matrix::{Companion, Handle, PublishedBundle};

/// Fallible public fixture and assertion result.
pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A coherent voltage difference with a named physical kind.
pub(crate) fn voltage() -> Result<Descriptor, nrz_units::UnitError> {
    Descriptor::new(
        Kind::Named(
            "voltage".to_owned(),
            Box::new(Dimension::integers([2, 1, -3, -1, 0, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Difference,
    )
}

/// Require the complete bound handle promised by a declared-bound import.
pub(crate) fn bound<P: Scalar>(
    bundle: PublishedBundle<P>,
) -> Result<Handle<Companion, P>, std::io::Error> {
    bundle
        .bound()
        .ok_or_else(|| std::io::Error::other("declared bound was not published"))
}
