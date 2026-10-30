// SPDX-License-Identifier: AGPL-3.0-only
//! Declared torus and phase fixtures retain closed original boxes and exact affine transport.

use super::{
    decision_fixture::{cycle, ledger},
    lattice_fixture::{self, LatticeFixture, term},
    lattice_physical,
};
use crate::binding_testing::TestResult;
use nrz_bindings::readings::algebra::{
    Rational,
    decision::{EnclosureOrigin, RationalEnclosure},
    lattice::{LatticeBoxEnclosure, LatticeEnclosure, LatticeSelection, RationalBoxEnclosure},
    proof::{Statement, VerifiedWitness},
    syntax::{AtomicSort, Operator, Sort},
};
use nrz_bindings::{LatticeDecisionCase, SelectedLattice};
use std::sync::{Arc, Mutex};

/// Original mathematical graph, physical declaration and complete candidate evidence.
pub(super) struct Selection {
    /// Original graph remains mutable until the portable selected outputs are constructed.
    pub(super) fixture: LatticeFixture,
    /// Complete checked physical correspondence.
    pub(super) case: LatticeDecisionCase,
    /// Independently checked finite affine cover.
    pub(super) cover: VerifiedWitness,
    /// Named complete candidate, with no authority before selection.
    pub(super) expected: LatticeSelection,
    /// Whole original-box transport, retaining each original coordinate.
    pub(super) enclosure: LatticeEnclosure,
}

/// Prepare the phase or declared torus, optionally widening only the first torus radius.
pub(super) fn prepared(kind: u8, wide: bool) -> TestResult<Selection> {
    let mut fixture = lattice_fixture::lattice(kind)?;
    let owner = lattice_physical::source(kind, "1", "live closed lattice")?;
    let spec = lattice_physical::specification(&mut fixture, kind, &owner)?;
    let case = LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut ledger()?)?;
    let cover =
        fixture
            .frame
            .cas
            .prove_lattice_cover(&fixture.basis, &fixture.index, &mut ledger()?)?;
    let Statement::LatticeCover(value) = &cover.judgment().statement else {
        return Err("missing affine cover".into());
    };
    let cover_value = *value.clone();
    let enclosure = enclosure(&fixture, kind, wide)?;
    let selected = if kind == 0 { vec![1] } else { vec![3, 1] };
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

/// Original target intervals include the scalar primitive's conservative outward rounding.
fn original(fixture: &LatticeFixture, kind: u8, wide: bool) -> TestResult<RationalBoxEnclosure> {
    let targets = fixture
        .frame
        .cas
        .operands(&fixture.target, &mut ledger()?)?;
    let context = fixture.frame.context.clone();
    let input_family = fixture.frame.cas.admissible_region(&context)?;
    let mut coordinates = Vec::new();
    for (axis, target) in targets.into_iter().enumerate() {
        let (lower, upper) = if kind == 0 {
            ("15/2", "8")
        } else if axis == 1 {
            ("899/100", "1001/100")
        } else if wide {
            ("1799/100", "2101/100")
        } else {
            ("1899/100", "2001/100")
        };
        let origin = if kind == 0 {
            EnclosureOrigin::PiBound {
                expression: target.clone(),
                certificate: Box::new(cycle()?),
            }
        } else {
            EnclosureOrigin::ExactRational {
                expression: target.clone(),
                value: Rational::from_ratio(if axis == 0 { 20 } else { 10 }, 1, &mut ledger()?)?,
            }
        };
        coordinates.push(RationalEnclosure {
            target,
            context: context.clone(),
            input_family: input_family.clone(),
            lower: Rational::parse(lower, &mut ledger()?)?,
            upper: Rational::parse(upper, &mut ledger()?)?,
            origin,
        });
    }
    Ok(RationalBoxEnclosure {
        target: fixture.target.clone(),
        context,
        input_family,
        coordinates,
    })
}

/// Exact inverse coefficients keep Machin evidence and transport all original endpoints.
fn enclosure(fixture: &LatticeFixture, kind: u8, wide: bool) -> TestResult<LatticeEnclosure> {
    let original = original(fixture, kind, wide)?;
    let count = original.coordinates.len();
    let inverse = fixture.basis.inverse().map().expression().clone();
    let entries = fixture.frame.cas.operands(&inverse, &mut ledger()?)?;
    let mut coefficients = Vec::new();
    for (position, target) in entries.into_iter().take(count * (count + 1)).enumerate() {
        let diagonal = position / (count + 1) == position % (count + 1);
        let (lower, upper, origin) = if diagonal {
            (
                Rational::parse("203/1280", &mut ledger()?)?,
                Rational::parse("205/1280", &mut ledger()?)?,
                EnclosureOrigin::PiBound {
                    expression: target.clone(),
                    certificate: Box::new(cycle()?),
                },
            )
        } else {
            (
                Rational::zero(),
                Rational::zero(),
                EnclosureOrigin::ExactRational {
                    expression: target.clone(),
                    value: Rational::zero(),
                },
            )
        };
        coefficients.push(RationalEnclosure {
            target,
            context: original.context.clone(),
            input_family: original.input_family.clone(),
            lower,
            upper,
            origin,
        });
    }
    let limits = if kind == 0 {
        vec![("1", "3/2")]
    } else {
        vec![(if wide { "14/5" } else { "3" }, "17/5"), ("7/5", "17/10")]
    };
    let bounds = limits
        .into_iter()
        .map(|(lower, upper)| {
            Ok((
                Rational::parse(lower, &mut ledger()?)?,
                Rational::parse(upper, &mut ledger()?)?,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    Ok(LatticeEnclosure::Original(Box::new(LatticeBoxEnclosure {
        basis: fixture.basis.question().clone(),
        original,
        inverse,
        coefficients,
        bounds,
    })))
}

/// Borrow the exact original target box from this fixture's supported transport form.
pub(super) fn original_box(value: &Selection) -> TestResult<&RationalBoxEnclosure> {
    let LatticeEnclosure::Original(enclosure) = &value.enclosure else {
        return Err("missing original box".into());
    };
    Ok(&enclosure.original)
}

/// Construct one complete mathematical and physical selection before any live publication.
pub(super) fn select(mut value: Selection) -> TestResult<SelectedLattice> {
    let witness = value.fixture.frame.cas.prove_lattice_selection(
        &value.cover,
        &value.expected,
        &value.enclosure,
        &mut ledger()?,
    )?;
    let outputs = value
        .fixture
        .frame
        .cas
        .lattice_selection_outputs(&witness, &mut ledger()?)?;
    Ok(value.case.admit(
        Arc::new(Mutex::new(value.fixture.frame.cas)),
        &outputs,
        value.case.attachment(),
        &mut ledger()?,
    )?)
}
