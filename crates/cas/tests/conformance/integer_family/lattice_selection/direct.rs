// SPDX-License-Identifier: AGPL-3.0-only
//! A sharper original affine certificate is separate from transporting a wider source box.

use super::{
    EnclosureOrigin, LatticeEnclosure, Operator, Rational, RationalBoxEnclosure, RationalEnclosure,
    Selection, TestResult, column, ledger, prepared, real, selected,
};
use nrz_cas::lattice::LatticeCoordinateEnclosure;
use nrz_cas::limits::Limits;
use nrz_cas::{Cas, InvalidCode};

pub(super) fn direct(kind: u32) -> TestResult<Selection> {
    let mut source = prepared(kind)?;
    let LatticeEnclosure::Original(affine) = &source.enclosure else {
        return Err("missing original box".into());
    };
    let entries = source
        .source
        .frame
        .cas
        .operands(&affine.inverse, &mut ledger()?)?;
    let count = affine.original.coordinates.len();
    let mut coordinates = Vec::new();
    for row in 0..count {
        let start = row * (count + 1);
        let mut target = entries[start + count].clone();
        for (column, original) in affine.original.coordinates.iter().enumerate() {
            let product = source.source.frame.term(
                Operator::Mul,
                real(),
                &[entries[start + column].clone(), original.target.clone()],
            )?;
            target = source
                .source
                .frame
                .term(Operator::Add, real(), &[target, product])?;
        }
        let (lower, upper, origin) = origin(kind, row, &target)?;
        coordinates.push(RationalEnclosure {
            target,
            context: affine.original.context.clone(),
            input_family: affine.original.input_family.clone(),
            lower,
            upper,
            origin,
        });
    }
    let targets = coordinates
        .iter()
        .map(|entry| entry.target.clone())
        .collect::<Vec<_>>();
    let context = affine.original.context.clone();
    let input_family = affine.original.input_family.clone();
    let basis = affine.basis.clone();
    let target = affine.original.target.clone();
    let inverse = affine.inverse.clone();
    let transformed = column(&mut source.source, &targets)?;
    source.enclosure = LatticeEnclosure::Coordinates(Box::new(LatticeCoordinateEnclosure {
        basis,
        target,
        inverse,
        coordinates: RationalBoxEnclosure {
            target: transformed,
            context,
            input_family,
            coordinates,
        },
    }));
    Ok(source)
}

fn origin(
    kind: u32,
    row: usize,
    target: &nrz_cas::Handle,
) -> TestResult<(Rational, Rational, EnclosureOrigin)> {
    Ok(if matches!(kind, 2 | 3 | 5) {
        let (numerator, denominator) = match kind {
            2 => (5, 4),
            3 => (1, 1),
            _ => (if row == 0 { 12 } else { 16 }, 5),
        };
        let value = Rational::from_ratio(numerator, denominator, &mut ledger()?)?;
        (
            value.clone(),
            value.clone(),
            EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value,
            },
        )
    } else {
        let denominator = if row == 1 { 128 } else { 64 };
        (
            Rational::from_ratio(203, denominator, &mut ledger()?)?,
            Rational::from_ratio(205, denominator, &mut ledger()?)?,
            EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(crate::conformance::pi_bounds::cycle()?),
            },
        )
    })
}

/// Exact original affine replay selects the owned wrap face and the direct torus, phase, LC and grid.
/// ´test:integration:lattice-tuple-direct-selection´
#[test]
fn lattice_tuple_direct_selection() -> TestResult {
    for kind in 0..6 {
        let mut source = direct(kind)?;
        let witness = selected(&mut source, &mut ledger()?)?;
        let bytes = source
            .source
            .frame
            .cas
            .export_witness(&witness, &mut ledger()?)?;
        let mut receiver = Cas::new(Limits::default())?;
        let replayed = receiver.inspect(&bytes, &mut ledger()?)?;
        assert_eq!(receiver.export_witness(&replayed, &mut ledger()?)?, bytes);
    }
    Ok(())
}

/// A standalone literal cannot impersonate the original affine expression, even with the same value.
/// ´test:integration:lattice-tuple-direct-refusal´
#[test]
fn lattice_tuple_direct_refusal() -> TestResult {
    let mut source = direct(3)?;
    let surrogate = super::number(&mut source.source, 1, 1, false)?;
    let transformed = column(&mut source.source, std::slice::from_ref(&surrogate))?;
    let LatticeEnclosure::Coordinates(enclosure) = &mut source.enclosure else {
        return Err("missing direct box".into());
    };
    enclosure.coordinates.target = transformed;
    enclosure.coordinates.coordinates[0].target = surrogate.clone();
    enclosure.coordinates.coordinates[0].origin = EnclosureOrigin::ExactRational {
        expression: surrogate,
        value: Rational::one(),
    };
    super::refuses(
        &mut source,
        InvalidCode::EndpointMismatch,
        "lattice coordinate box enclosure",
    )
}
