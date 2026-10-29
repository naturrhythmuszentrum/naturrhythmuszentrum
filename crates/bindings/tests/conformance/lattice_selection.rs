// SPDX-License-Identifier: AGPL-3.0-only
//! Public CAS selections feed physical outputs without inferring a candidate from storage.

use super::{
    decision_fixture::{cycle, ledger},
    lattice_fixture::{LatticeFixture, column_or_matrix, lattice, term},
    lattice_physical::{source, specification},
};
use crate::testing::{declarations::TestResult, readings::real};
use nrz_bindings::{LatticeDecisionCase, LatticeMetric, SelectedLattice};
use nrz_cas::{
    Handle, Rational,
    decision::{EnclosureOrigin, RationalEnclosure},
    lattice::{
        LatticeConvention, LatticeCoordinateEnclosure, LatticeEnclosure, LatticeSelection,
        RationalBoxEnclosure,
    },
    proof::{Statement, VerifiedWitness},
    syntax::{AtomicSort, Operator, Sort},
};
use std::rc::Rc;

/// Exact original question, checked cover and closed original affine enclosure.
pub(super) struct Selection {
    /// Complete mutable original mathematical graph before publishing a physical snapshot.
    pub(super) fixture: LatticeFixture,
    /// Original physically checked question.
    pub(super) case: LatticeDecisionCase,
    /// Portable admitted affine cover.
    pub(super) cover: VerifiedWitness,
    /// Named exact mathematical tuple.
    pub(super) expected: LatticeSelection,
    /// Closed original affine evidence, retaining its source rather than a surrogate literal.
    pub(super) enclosure: LatticeEnclosure,
}

/// Prepare phase, two-angle and grid fixtures with explicit floor or nearest convention.
pub(super) fn prepared(kind: u8, convention: LatticeConvention) -> TestResult<Selection> {
    let mut fixture = lattice(kind)?;
    let owner = source(kind, "1", "original selected coordinates")?;
    let mut spec = specification(&mut fixture, kind, &owner)?;
    spec.convention = convention;
    spec.metric = if convention == LatticeConvention::Nearest {
        Some(LatticeMetric::EuclideanNormalized)
    } else {
        None
    };
    let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut ledger()?)?;
    let cover =
        fixture
            .frame
            .cas
            .prove_lattice_cover(&fixture.basis, &fixture.index, &mut ledger()?)?;
    let Statement::LatticeCover(value) = &cover.judgment().statement else {
        return Err("missing lattice cover".into());
    };
    let cover_value = *value.clone();
    let enclosure = enclosure(&mut fixture, kind)?;
    let selected = match kind {
        0 => vec![1],
        1 => vec![
            3,
            if convention == LatticeConvention::Nearest {
                2
            } else {
                1
            },
        ],
        _ => vec![2, 3],
    };
    let mut values = Vec::new();
    for value in selected {
        values.push(term(
            &mut fixture.frame,
            Operator::Rational(Rational::from_ratio(value, 1, &mut ledger()?)?),
            Sort::Value(AtomicSort::Integer),
            &[],
        )?);
    }
    let expected = LatticeSelection {
        cover: cover_value,
        target: fixture.target.clone(),
        values,
    };
    Ok(Selection {
        fixture,
        case,
        cover,
        expected,
        enclosure,
    })
}

/// Replay each exact original inverse expression; canceled Pi stays in the expression graph.
fn enclosure(fixture: &mut LatticeFixture, kind: u8) -> TestResult<LatticeEnclosure> {
    let inverse = fixture.basis.inverse().map().expression().clone();
    let coefficients = fixture.frame.cas.operands(&inverse, &mut ledger()?)?;
    let originals = fixture
        .frame
        .cas
        .operands(&fixture.target, &mut ledger()?)?;
    let count = originals.len();
    let context = fixture.frame.context.clone();
    let input_family = fixture.frame.cas.admissible_region(&context)?;
    let mut coordinates = Vec::new();
    for row in 0..count {
        let start = row * (count + 1);
        let mut target = coefficients[start + count].clone();
        for (column, original) in originals.iter().enumerate() {
            let product = term(
                &mut fixture.frame,
                Operator::Mul,
                real(),
                &[coefficients[start + column].clone(), original.clone()],
            )?;
            target = term(
                &mut fixture.frame,
                Operator::Add,
                real(),
                &[target, product],
            )?;
        }
        let (lower, upper, origin) = bounds(kind, row, &target)?;
        coordinates.push(RationalEnclosure {
            target,
            context: context.clone(),
            input_family: input_family.clone(),
            lower,
            upper,
            origin,
        });
    }
    let values = coordinates
        .iter()
        .map(|entry| entry.target.clone())
        .collect::<Vec<_>>();
    let target = column_or_matrix(&mut fixture.frame, u32::try_from(count)?, 1, &values)?;
    Ok(LatticeEnclosure::Coordinates(Box::new(
        LatticeCoordinateEnclosure {
            basis: fixture.basis.question().clone(),
            target: fixture.target.clone(),
            inverse,
            coordinates: RationalBoxEnclosure {
                target,
                context,
                input_family,
                coordinates,
            },
        },
    )))
}

/// Rational direct origins and the existing independent finite Machin certificate.
fn bounds(
    kind: u8,
    row: usize,
    target: &Handle,
) -> TestResult<(Rational, Rational, EnclosureOrigin)> {
    if kind == 1 {
        let (lower, upper) = if row == 0 {
            ("203/64", "205/64")
        } else {
            ("203/128", "205/128")
        };
        Ok((
            Rational::parse(lower, &mut ledger()?)?,
            Rational::parse(upper, &mut ledger()?)?,
            EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(cycle()?),
            },
        ))
    } else {
        let value = Rational::parse(
            if kind == 0 {
                "5/4"
            } else if row == 0 {
                "12/5"
            } else {
                "16/5"
            },
            &mut ledger()?,
        )?;
        Ok((
            value.clone(),
            value.clone(),
            EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value,
            },
        ))
    }
}

/// Borrow the unchanged CAS producer for the declared convention.
pub(super) fn select(value: &mut Selection) -> TestResult<VerifiedWitness> {
    Ok(match value.case.specification().convention {
        LatticeConvention::Floor => value.fixture.frame.cas.prove_lattice_selection(
            &value.cover,
            &value.expected,
            &value.enclosure,
            &mut ledger()?,
        )?,
        LatticeConvention::Nearest => value.fixture.frame.cas.prove_lattice_nearest_selection(
            &value.cover,
            &value.expected,
            &value.enclosure,
            &mut ledger()?,
        )?,
    })
}

/// Complete physical admission leaves all mathematical production charges outside this owner.
pub(super) fn admitted(kind: u8, convention: LatticeConvention) -> TestResult<SelectedLattice> {
    let mut value = prepared(kind, convention)?;
    let witness = select(&mut value)?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    Ok(value.case.admit(
        Rc::new(value.fixture.frame.cas),
        &outputs,
        value.case.attachment(),
        &mut ledger()?,
    )?)
}
