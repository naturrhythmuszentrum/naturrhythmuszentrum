// SPDX-License-Identifier: AGPL-3.0-only
//! Closed physical originals and exact normalization for the question constructor.

use super::{decision_fixture::ledger, lattice_fixture::LatticeFixture};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    CheckedDeclarations, Context, DecisionCoordinate, Declaration, Descriptor, Evidence,
    ExponentPolicy, LatticeAxis, LatticeDecisionSpec, LatticeIndexKind, Limits, Measure,
    ReferenceDefinition, Registry, Role,
    readings::{CoordinateNormalization, EntryMeaning},
};
use nrz_cas::{Rational, lattice::LatticeConvention};
use std::rc::Rc;

/// Exact declared phase or grid coordinates, with an independently replaceable full-turn source.
pub(super) fn source(
    kind: u8,
    period: &str,
    provenance: &str,
) -> TestResult<Rc<CheckedDeclarations>> {
    let registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    let angle = registry.measure("rad").ok_or("missing radian")?.clone();
    let turn = registry.measure("turn").ok_or("missing turn")?.clone();
    let ratio = Descriptor::ratio();
    let values = match kind {
        0 => vec![
            ("axis_a", turn.clone(), "5/4"),
            ("period0", turn, period),
            ("zero", angle, "0"),
        ],
        1 => vec![
            ("axis_a", angle.clone(), "20"),
            ("axis_b", angle.clone(), "10"),
            ("period0", turn.clone(), period),
            ("period1", turn, period),
            ("zero", angle, "0"),
        ],
        _ => vec![
            ("axis_a", ratio.clone(), "3/10"),
            ("axis_b", ratio.clone(), "1/5"),
            ("period0", ratio.clone(), "1/8"),
            ("period1", ratio.clone(), "1/16"),
            ("zero", ratio, "0"),
        ],
    };
    let names = values.iter().map(|(name, _, _)| *name).collect::<Vec<_>>();
    let mut owner = Context::with_real_symbols(registry, Limits::default(), &names)?;
    for (name, descriptor, value) in values {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor,
                value: Rational::parse(value, &mut ledger()?)?,
                evidence: Evidence {
                    source: provenance.into(),
                    assurance: "exact declared coordinate".into(),
                },
            },
        )?;
    }
    Ok(Rc::new(owner.freeze()?))
}

/// Retain exact physical maps and the declared Integer meanings before any selection.
pub(super) fn specification(
    fixture: &mut LatticeFixture,
    kind: u8,
    owner: &Rc<CheckedDeclarations>,
) -> TestResult<LatticeDecisionSpec> {
    let count = if kind == 0 { 1 } else { 2 };
    let mut axes = Vec::new();
    let mut meanings = Vec::new();
    let mut indices = Vec::new();
    let zero = DecisionCoordinate::declared(Rc::clone(owner), "zero", &mut ledger()?)?;
    for row in 0..count {
        let name = ["axis_a", "axis_b"][row].to_owned();
        let target = DecisionCoordinate::declared(Rc::clone(owner), &name, &mut ledger()?)?;
        let period = DecisionCoordinate::declared(
            Rc::clone(owner),
            &format!("period{row}"),
            &mut ledger()?,
        )?;
        let descriptor = Descriptor::new(
            target.checked()?.descriptor().kind().clone(),
            Measure::coherent(),
            Role::Difference,
        )?;
        let reference = ReferenceDefinition::unit(&name, &descriptor)?;
        meanings.push(EntryMeaning::new(
            descriptor,
            reference,
            None,
            &mut ledger()?,
        )?);
        let mut basis = vec![zero.clone(); count];
        basis[row] = period;
        axes.push(LatticeAxis {
            name: name.clone(),
            target,
            offset: zero.clone(),
            basis,
        });
        indices.push(if kind == 2 {
            LatticeIndexKind::QuantizationIndex(name)
        } else {
            LatticeIndexKind::SignedWinding(name)
        });
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
