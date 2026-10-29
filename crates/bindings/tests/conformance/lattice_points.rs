// SPDX-License-Identifier: AGPL-3.0-only
//! Nonunit physical references restore points and basis differences with separate affine roles.

use super::{
    decision_fixture::ledger,
    lattice_fixture::{LatticeFixture, lattice},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    CheckedDeclarations, Context, DecisionCoordinate, DecisionError, Declaration, Descriptor,
    Dimension, Evidence, ExponentPolicy, Kind, LatticeAxis, LatticeDecisionCase,
    LatticeDecisionSpec, LatticeIndexKind, Limits, Measure, ReferenceDefinition, Registry, Role,
    Scale,
    readings::{CoordinateNormalization, EntryMeaning},
};
use nrz_cas::{Rational, lattice::LatticeConvention};
use std::rc::Rc;

/// Exact points use two different origins and scales; basis columns remain physical differences.
/// ´claim:bindings:lattice-point-restoration´
/// ´test:integration:bindings-lattice-point-restoration´
#[test]
fn bindings_lattice_point_restoration() -> TestResult {
    let mut fixture = lattice(2)?;
    let spec = specification(&mut fixture)?;
    let before = fixture.frame.cas.counts();
    let original = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut ledger()?)?;
    assert_eq!(
        original.specification().frame.entries()[0]
            .descriptor()
            .role(),
        Role::Point
    );
    let inputs = original.associate_inputs(&mut ledger()?)?;
    assert_eq!(inputs.operands().len(), 7);
    let mut point_basis = spec.clone();
    point_basis.axes[0].basis[0] = spec.axes[0].target.clone();
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, point_basis, &mut ledger()?),
        Err(DecisionError::LatticeMeaning)
    ));
    let mut difference_offset = spec;
    difference_offset.axes[0].offset = difference_offset.axes[0].basis[1].clone();
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, difference_offset, &mut ledger()?),
        Err(DecisionError::LatticeMeaning)
    ));
    assert_eq!(fixture.frame.cas.counts(), before);
    Ok(())
}

/// The normalized grid restores through S=diag(2,3) and o=(10,20) in metres.
pub(super) fn specification(fixture: &mut LatticeFixture) -> TestResult<LatticeDecisionSpec> {
    let owner = source()?;
    let zero = DecisionCoordinate::declared(Rc::clone(&owner), "zero", &mut ledger()?)?;
    let mut axes = Vec::new();
    let mut meanings = Vec::new();
    let mut indices = Vec::new();
    for (row, scale) in [(0, "2"), (1, "3")] {
        let name = format!("position{row}");
        let target = DecisionCoordinate::declared(Rc::clone(&owner), &name, &mut ledger()?)?;
        let offset_name = format!("offset{row}");
        let offset = DecisionCoordinate::declared(Rc::clone(&owner), &offset_name, &mut ledger()?)?;
        let period = DecisionCoordinate::declared(
            Rc::clone(&owner),
            ["increment_x", "increment_y"][row],
            &mut ledger()?,
        )?;
        let reference = ReferenceDefinition::new(
            &name,
            period.checked()?.descriptor(),
            Scale::Exact(Rational::parse(scale, &mut ledger()?)?),
        )?;
        meanings.push(EntryMeaning::new(
            target.checked()?.descriptor().clone(),
            reference,
            Some(owner.origin_definition(&offset_name)?),
            &mut ledger()?,
        )?);
        let mut basis = vec![zero.clone(); 2];
        basis[row] = period;
        axes.push(LatticeAxis {
            name: name.clone(),
            target,
            offset,
            basis,
        });
        indices.push(LatticeIndexKind::QuantizationIndex(name));
    }
    let frame = CoordinateNormalization::new(
        &mut fixture.frame.cas,
        &fixture.frame.context,
        &meanings,
        &mut ledger()?,
    )?;
    Ok(LatticeDecisionSpec {
        basis: Rc::clone(&fixture.basis),
        index: Rc::clone(&fixture.index),
        frame,
        axes,
        indices,
        target: fixture.target.clone(),
        convention: LatticeConvention::Floor,
        metric: None,
    })
}

/// Original point coordinates and linear increments have the same length kind but distinct roles.
fn source() -> TestResult<Rc<CheckedDeclarations>> {
    let length = Kind::Named(
        "length".into(),
        Box::new(Dimension::integers([1, 0, 0, 0, 0, 0, 0])?),
    );
    let point = Descriptor::new(length.clone(), Measure::coherent(), Role::Point)?;
    let difference = Descriptor::new(length, Measure::coherent(), Role::Difference)?;
    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        Limits::default(),
        &[
            "position0",
            "position1",
            "offset0",
            "offset1",
            "increment_x",
            "increment_y",
            "zero",
        ],
    )?;
    for (name, descriptor, value) in [
        ("position0", point.clone(), "53/5"),
        ("position1", point.clone(), "103/5"),
        ("offset0", point.clone(), "10"),
        ("offset1", point, "20"),
        ("increment_x", difference.clone(), "1/4"),
        ("increment_y", difference.clone(), "3/16"),
        ("zero", difference, "0"),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor,
                value: Rational::parse(value, &mut ledger()?)?,
                evidence: Evidence::default(),
            },
        )?;
    }
    Ok(Rc::new(owner.freeze()?))
}
