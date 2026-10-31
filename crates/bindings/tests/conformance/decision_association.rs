// SPDX-License-Identifier: AGPL-3.0-only
//! Physical original associations are distinct from equal numerical declarations.

use super::{
    decision_admission::{physical_source, specification},
    decision_fixture::{fixture, ledger},
};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    BindingSymbols, CountDomain, DecisionError, DecisionExpression, DecisionOperand,
    IntegerDecisionCase,
};

/// Original LC duration/period source, case and parameter instance.
fn case(source: &str, doubled_unit: bool, variant: u8) -> TestResult<IntegerDecisionCase> {
    let source_owner = physical_source(source, doubled_unit)?;
    let source = fixture(false, "3", "10/Pi")?;
    let physical = DecisionExpression::declared(source_owner, "quotient", &mut ledger()?)?;
    let mut specification = specification(&source, CountDomain::NONNEGATIVE)?;
    if variant == 1 {
        specification.family.lower = None;
    } else if variant == 2 {
        specification.parameters.push(
            specification
                .family
                .lower
                .clone()
                .ok_or("missing Integer parameter")?,
        );
    }
    Ok(IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification,
        &mut ledger()?,
    )?)
}

/// Measure new association construction and complete identity checking independently.
/// ´test:integration:decision-association-costs´
#[test]
fn decision_association_costs() -> TestResult {
    let case = case("original LC source", false, 0)?;
    let mut budget = ledger()?;
    let associated = case.associate_inputs(&mut budget)?;
    let construction = budget.usage().work;
    let symbols = associated
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    associated.authenticate(&case, &symbols, &mut budget)?;
    assert_eq!(
        associated
            .operands()
            .iter()
            .map(DecisionOperand::name)
            .collect::<Vec<_>>(),
        ["duration", "period"]
    );
    assert_ne!(
        associated.operands()[1].original()?.descriptor(),
        symbols[1].value().descriptor()
    );
    assert_eq!(construction, 22_693);
    assert_eq!(budget.usage().work - construction, 8);
    assert_eq!(associated.retained_payload_bytes(), 8996);
    Ok(())
}

/// A fresh equal symbol bundle or changed original declaration cannot replace the association.
/// ´test:integration:decision-association-refusals´
#[test]
fn decision_association_refusals() -> TestResult {
    let original = case("original LC source", false, 0)?;
    let associated = original.associate_inputs(&mut ledger()?)?;
    let symbols = associated
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    for other in [
        case("changed source", false, 0)?,
        case("original LC source", true, 0)?,
        case("original LC source", false, 1)?,
        case("original LC source", false, 2)?,
    ] {
        assert!(matches!(
            associated.authenticate(&other, &symbols, &mut ledger()?),
            Err(DecisionError::Attachment)
        ));
    }
    let independent = BindingSymbols::declare(
        symbols[0].value().descriptor(),
        [1, 1],
        [true, true],
        &mut ledger()?,
    )?;
    assert!(matches!(
        associated.authenticate(&original, &[&independent, symbols[1]], &mut ledger()?),
        Err(DecisionError::Association)
    ));
    assert!(matches!(
        associated.authenticate(&original, &[symbols[1], symbols[0]], &mut ledger()?),
        Err(DecisionError::Association)
    ));
    assert!(matches!(
        associated.authenticate(&original, &symbols[..1], &mut ledger()?),
        Err(DecisionError::Association)
    ));
    associated.authenticate(&original, &symbols, &mut ledger()?)?;
    Ok(())
}

/// New association Work is cumulative, including usage spent before either operation.
/// ´test:integration:decision-association-work-boundaries´
/// ´claim:bindings:decision-association-work-boundaries´ Frozen source environments reduce cumulative association Work from 24428 to 22710 while one-below refusal preserves its source.
#[test]
fn decision_association_work_boundaries() -> TestResult {
    let original = case("original LC source", false, 0)?;
    let before = original.source().owner().cas()?.counts();
    for (work, accepted) in [(22_710, true), (22_709, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(17, nrz_cas::Location::new("prior association caller"))?;
        let result = original.associate_inputs(&mut budget);
        if accepted {
            assert_eq!(result?.operands().len(), 2);
            assert_eq!(budget.usage().work, 22_710);
        } else {
            let Err(DecisionError::Symbols(nrz_bindings::AdmissionError::Algebra(fault))) = result
            else {
                panic!("one-below association Work must refuse");
            };
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: nrz_cas::limits::BudgetKind::Work,
                    ..
                }
            ));
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
        assert_eq!(original.source().owner().cas()?.counts(), before);
    }
    let association = original.associate_inputs(&mut ledger()?)?;
    let symbols = association
        .operands()
        .iter()
        .map(DecisionOperand::symbols)
        .collect::<Vec<_>>();
    for (work, accepted) in [(25, true), (24, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(17, nrz_cas::Location::new("prior association caller"))?;
        let result = association.authenticate(&original, &symbols, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(budget.usage().work, 25);
        }
        let monotone = budget.usage().work >= 17 && budget.usage().work <= work;
        assert!(monotone);
        assert_eq!(original.source().owner().cas()?.counts(), before);
    }
    Ok(())
}

/// The original check retains operands that exact coordinate normalization cancels.
/// ´test:integration:decision-association-original-closure´
#[test]
fn decision_association_original_closure() -> TestResult {
    use nrz_bindings::{
        Context, Declaration, Descriptor, Evidence, ExponentPolicy, Observable, Registry,
    };
    use nrz_cas::Rational;
    use std::rc::Rc;

    let mut owner = Context::with_real_symbols(
        Registry::new(ExponentPolicy::Rational),
        nrz_bindings::Limits::default(),
        &["a", "b", "c", "quotient"],
    )?;
    for (name, value) in [("a", 3), ("b", 4), ("c", 5)] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor: Descriptor::ratio(),
                value: Rational::from_ratio(value, 1, &mut ledger()?)?,
                evidence: Evidence::default(),
            },
        )?;
    }
    let expression = owner.parse("complete original", "a+(b-b)+0*c")?;
    owner.declare(
        "quotient".into(),
        Declaration::Derived(Observable {
            descriptor: Descriptor::ratio(),
            expression,
            evidence: Evidence::default(),
        }),
    )?;
    let physical =
        DecisionExpression::declared(Rc::new(owner.freeze()?), "quotient", &mut ledger()?)?;
    let source = fixture(false, "3", "3")?;
    let original = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        &mut ledger()?,
    )?;
    let association = original.associate_inputs(&mut ledger()?)?;
    assert_eq!(
        association
            .operands()
            .iter()
            .map(DecisionOperand::name)
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    Ok(())
}
