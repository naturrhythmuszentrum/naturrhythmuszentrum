// SPDX-License-Identifier: AGPL-3.0-only
//! Question and input-association mutations refuse at their physical identity owners.

use super::{
    decision_fixture::ledger,
    lattice_fixture::lattice,
    lattice_physical::{source, specification},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    BindingSymbols, CountDomain, DecisionCoordinate, DecisionError, DecisionOperand, Descriptor,
    Kind, LatticeDecisionCase, LatticeIndexKind, LatticeMetric, Measure, Role,
};
use nrz_cas::lattice::LatticeConvention;
use std::rc::Rc;

/// Missing axes, wrong matrix roles and malformed metrics cannot become physical questions.
/// ´claim:bindings:lattice-question-refusals´
/// ´test:integration:bindings-lattice-question-refusals´
#[test]
fn bindings_lattice_question_refusals() -> TestResult {
    let mut fixture = lattice(2)?;
    let owner = source(2, "1", "grid source")?;
    let spec = specification(&mut fixture, 2, &owner)?;
    let before = fixture.frame.cas.counts();
    let mut missing = spec.clone();
    missing.axes.pop();
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, missing, &mut ledger()?),
        Err(DecisionError::LatticeShape)
    ));
    let mut repeated = spec.clone();
    repeated.axes[1].name.clone_from(&spec.axes[0].name);
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, repeated, &mut ledger()?),
        Err(DecisionError::LatticeShape)
    ));
    let mut target = spec.clone();
    target.target = fixture.frame.context.clone();
    assert!(
        matches!(LatticeDecisionCase::declare(&fixture.frame.cas, target, &mut ledger()?), Err(DecisionError::Algebra(fault)) if fault.kind == nrz_cas::FaultKind::Invalid(nrz_cas::InvalidCode::Reference))
    );
    for convention in [LatticeConvention::Floor, LatticeConvention::Nearest] {
        let mut wrong = spec.clone();
        wrong.convention = convention;
        wrong.metric = if convention == LatticeConvention::Floor {
            Some(LatticeMetric::EuclideanNormalized)
        } else {
            None
        };
        assert!(matches!(
            LatticeDecisionCase::declare(&fixture.frame.cas, wrong, &mut ledger()?),
            Err(DecisionError::LatticeMetric)
        ));
    }
    assert_eq!(fixture.frame.cas.counts(), before);
    Ok(())
}

/// A count declaration needs a named nonnegative domain; ordinary indices stay signed.
/// ´claim:bindings:lattice-index-kinds´
/// ´test:integration:bindings-lattice-index-kinds´
#[test]
fn bindings_lattice_index_kinds() -> TestResult {
    let mut fixture = lattice(2)?;
    let owner = source(2, "1", "grid source")?;
    let spec = specification(&mut fixture, 2, &owner)?;
    for kind in [
        LatticeIndexKind::SignedWinding(String::new()),
        LatticeIndexKind::Count(Box::new(Descriptor::ratio())),
        LatticeIndexKind::Count(Box::new(Descriptor::new(
            Kind::Count("signed tally".into(), CountDomain::SIGNED),
            Measure::coherent(),
            Role::Difference,
        )?)),
    ] {
        let mut changed = spec.clone();
        changed.indices[0] = kind;
        assert!(matches!(
            LatticeDecisionCase::declare(&fixture.frame.cas, changed, &mut ledger()?),
            Err(DecisionError::LatticeIndexKind)
        ));
    }
    let mut counted = spec;
    counted.indices[0] = LatticeIndexKind::Count(Box::new(Descriptor::new(
        Kind::Count(
            "bounded samples".into(),
            CountDomain::new(Some(2), Some(9))?,
        ),
        Measure::coherent(),
        Role::Difference,
    )?));
    let case = LatticeDecisionCase::declare(&fixture.frame.cas, counted, &mut ledger()?)?;
    assert_eq!(case.specification().indices[0].name(), "bounded samples");
    Ok(())
}

/// Original zero entries retain their kinds, and nonzero offsets cannot replace declared zeros.
/// ´claim:bindings:lattice-original-coordinates´
/// ´test:integration:bindings-lattice-original-coordinates´
#[test]
fn bindings_lattice_original_coordinates() -> TestResult {
    let mut fixture = lattice(1)?;
    let owner = source(1, "1", "two phase sources")?;
    let spec = specification(&mut fixture, 1, &owner)?;
    let other = source(2, "1", "dimensionless zero")?;
    let mut changed = spec.clone();
    changed.axes[0].basis[1] = DecisionCoordinate::declared(other, "zero", &mut ledger()?)?;
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, changed, &mut ledger()?),
        Err(DecisionError::Units(nrz_bindings::UnitError::KindMismatch))
    ));
    let mut shifted = spec;
    shifted.axes[0].offset = DecisionCoordinate::declared(owner, "period0", &mut ledger()?)?;
    assert!(matches!(
        LatticeDecisionCase::declare(&fixture.frame.cas, shifted, &mut ledger()?),
        Err(DecisionError::LatticeMeaning)
    ));
    Ok(())
}

/// Shared leaves deduplicate by frozen source identity; equal foreign zeros remain separate originals.
/// ´claim:bindings:lattice-input-identities´
/// ´test:integration:bindings-lattice-input-identities´
#[test]
fn bindings_lattice_input_identities() -> TestResult {
    let mut fixture = lattice(2)?;
    let owner = source(2, "1", "grid source")?;
    let mut spec = specification(&mut fixture, 2, &owner)?;
    let original = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut ledger()?)?;
    let association = original.associate_inputs(&mut ledger()?)?;
    assert_eq!(
        association
            .operands()
            .iter()
            .map(DecisionOperand::name)
            .collect::<Vec<_>>(),
        ["axis_a", "zero", "period0", "axis_b", "period1"]
    );
    let other = source(2, "1", "independent zero source")?;
    spec.axes[1].offset = DecisionCoordinate::declared(Rc::clone(&other), "zero", &mut ledger()?)?;
    let changed = LatticeDecisionCase::declare(&fixture.frame.cas, spec, &mut ledger()?)?;
    assert!(matches!(
        changed.validate_attachment(&original.attachment(), &mut ledger()?),
        Err(DecisionError::Attachment)
    ));
    let changed_inputs = changed.associate_inputs(&mut ledger()?)?;
    assert_eq!(changed_inputs.operands().len(), 6);
    let zeros = changed_inputs
        .operands()
        .iter()
        .filter(|operand| operand.name() == "zero")
        .collect::<Vec<_>>();
    assert_eq!(zeros.len(), 2);
    assert_ne!(
        zeros[0].original()?.snapshot_id(),
        zeros[1].original()?.snapshot_id()
    );
    let symbols = association
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    assert!(matches!(
        association.authenticate(&changed, &symbols, &mut ledger()?),
        Err(DecisionError::Attachment)
    ));
    let replacement = BindingSymbols::declare(
        symbols[0].value().descriptor(),
        [1, 1],
        [true, true],
        &mut ledger()?,
    )?;
    let mut replaced = symbols.clone();
    replaced[0] = &replacement;
    assert!(matches!(
        association.authenticate(&original, &replaced, &mut ledger()?),
        Err(DecisionError::Association)
    ));
    assert!(matches!(
        association.authenticate(&original, &symbols[..4], &mut ledger()?),
        Err(DecisionError::Association)
    ));
    association.authenticate(&original, &symbols, &mut ledger()?)?;
    Ok(())
}

/// Moving the offset by one full basis step changes the question even with identical targets and sites.
/// ´claim:bindings:lattice-offset-identity´
/// ´test:integration:bindings-lattice-offset-identity´
#[test]
fn bindings_lattice_offset_identity() -> TestResult {
    let mut fixture = lattice(2)?;
    let owner = source(2, "1", "grid source")?;
    let spec = specification(&mut fixture, 2, &owner)?;
    let original = LatticeDecisionCase::declare(&fixture.frame.cas, spec.clone(), &mut ledger()?)?;
    super::lattice_fixture::translate(&mut fixture)?;
    let mut translated = spec;
    translated.basis = Rc::clone(&fixture.basis);
    translated.index = Rc::clone(&fixture.index);
    for (row, axis) in translated.axes.iter_mut().enumerate() {
        axis.offset = axis.basis[row].clone();
    }
    let translated = LatticeDecisionCase::declare(&fixture.frame.cas, translated, &mut ledger()?)?;
    assert_eq!(
        translated.specification().target,
        original.specification().target
    );
    assert_ne!(
        translated.question().basis.offset,
        original.question().basis.offset
    );
    assert!(matches!(
        translated.validate_attachment(&original.attachment(), &mut ledger()?),
        Err(DecisionError::Attachment)
    ));
    Ok(())
}
