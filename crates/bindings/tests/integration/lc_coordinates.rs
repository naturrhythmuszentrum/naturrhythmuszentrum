// SPDX-License-Identifier: AGPL-3.0-only
//! Typed fixed references compose the complete LC family before any reference-value substitution.

use crate::testing::lc_coordinates::{family, operator, term};
use crate::testing::normalization::{constant, entries};
use crate::testing::readings::{TestResult, ledger};
use nrz_bindings::{
    Descriptor, Dimension, Kind, Measure, ReferenceDefinition, Role, Scale,
    readings::{CoordinateNormalization, EntryMeaning},
};
use nrz_cas::{
    Rational,
    syntax::{Operator, Substitution},
};

/// The checked state inverse and typed time reference yield the independent whole-family operator.
/// ´test:integration:typed-lc-normalization-preserves-whole-family´
#[test]
fn typed_lc_normalization_preserves_whole_family() -> TestResult {
    let mut case = family()?;
    let frame =
        CoordinateNormalization::new(&mut case.cas, &case.context, &entries()?, &mut ledger()?)?;
    let quantity = Descriptor::new(
        Kind::Named(
            "time".into(),
            Box::new(Dimension::integers([0, 0, 1, 0, 0, 0, 0])?),
        ),
        Measure::coherent(),
        Role::Difference,
    )?;
    let reference =
        ReferenceDefinition::new("LC-time", &quantity, Scale::Expression(constant("1/2000")?))?;
    let time = EntryMeaning::new(quantity, reference, None, &mut ledger()?)?
        .magnitude()
        .clone();
    let time = case.cas.constant_reading_map(
        &case.context,
        (2, 2),
        &[time.clone(), constant("0")?, constant("0")?, time],
        &[constant("0")?, constant("0")?],
        &mut ledger()?,
    )?;
    let physical = operator(&mut case, "1", "-1")?;
    let right = case
        .cas
        .compose_reading_maps(&physical, frame.restoration(), &mut ledger()?)?;
    let similarity =
        case.cas
            .compose_reading_maps(frame.inverse().map(), &right, &mut ledger()?)?;
    let normalized = case
        .cas
        .compose_reading_maps(&time, &similarity, &mut ledger()?)?;
    let direct = operator(&mut case, "1/100000", "-1/40")?;
    let witness = case.cas.prove_reading_equality(
        &case.context,
        normalized.expression(),
        direct.expression(),
        &mut ledger()?,
    )?;
    assert!(witness.is_discharged());
    assert_eq!(witness.hypotheses().len(), 2);
    let mut substitutions = Vec::new();
    for (key, value) in [("L", "1/40"), ("C", "1/100000")] {
        let replacement = term(
            &mut case,
            Operator::Rational(Rational::parse(value, &mut ledger()?)?),
            &[],
        )?;
        substitutions.push(Substitution {
            key: key.into(),
            replacement,
        });
    }
    let actual = case
        .cas
        .substitute(normalized.expression(), &substitutions, &mut ledger()?)?;
    let rotation = case.cas.constant_reading_map(
        &case.context,
        (2, 2),
        &[
            constant("0")?,
            constant("1")?,
            constant("-1")?,
            constant("0")?,
        ],
        &[constant("0")?, constant("0")?],
        &mut ledger()?,
    )?;
    let witness = case.cas.prove_reading_equality(
        &case.context,
        &actual,
        rotation.expression(),
        &mut ledger()?,
    )?;
    assert!(witness.is_discharged());
    Ok(())
}
