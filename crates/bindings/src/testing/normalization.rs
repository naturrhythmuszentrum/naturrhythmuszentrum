// SPDX-License-Identifier: AGPL-3.0-only
//! Public coordinate-normalization fixtures, compiled only in the bindings public target.

use super::readings::{Fixture, TestResult, ledger};
use nrz_bindings::readings::{CoordinateNormalization, EntryMeaning};
use nrz_bindings::{Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role, Scale};
use nrz_cas::{ExactConstant, Rational};

/// Exact scalar spelling, independently of any normalization producer.
pub(crate) fn constant(text: &str) -> TestResult<ExactConstant> {
    Ok(Rational::parse(text, &mut ledger()?)?.into())
}

/// Explicit fixed voltage and current references in coherent difference coordinates.
pub(crate) fn entries() -> TestResult<Vec<EntryMeaning>> {
    let mut result = Vec::new();
    for (name, dimensions, magnitude) in [
        ("voltage", [2, 1, -3, -1, 0, 0, 0], "5"),
        ("current", [0, 0, 0, 1, 0, 0, 0], "1/10"),
    ] {
        let quantity = Descriptor::new(
            Kind::Named(name.into(), Box::new(Dimension::integers(dimensions)?)),
            Measure::coherent(),
            Role::Difference,
        )?;
        let reference =
            ReferenceDefinition::new(name, &quantity, Scale::Expression(constant(magnitude)?))?;
        result.push(EntryMeaning::new(
            quantity,
            reference,
            None,
            &mut ledger()?,
        )?);
    }
    Ok(result)
}

/// An independent exact graph with the fixed mixed coordinate contract.
pub(crate) fn frame() -> TestResult<(Fixture, CoordinateNormalization)> {
    let mut case = Fixture::new(&[])?;
    let frame =
        CoordinateNormalization::new(&mut case.cas, &case.context, &entries()?, &mut ledger()?)?;
    Ok((case, frame))
}

/// Compare complete row-major coefficients and offsets to independent rational spellings.
pub(crate) fn check(
    map: &nrz_cas::readings::AffineMap,
    scales: &[&str],
    shifts: &[&str],
) -> TestResult {
    let actual = map
        .coefficients()
        .iter()
        .map(|entry| entry.exact_string(&mut ledger()?))
        .collect::<Result<Vec<_>, _>>()?;
    let offsets = map
        .offsets()
        .iter()
        .map(|entry| entry.exact_string(&mut ledger()?))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(actual, scales);
    assert_eq!(offsets, shifts);
    Ok(())
}
