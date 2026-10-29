// SPDX-License-Identifier: AGPL-3.0-only
//! Complete nominal LC periods retain L, C, positive natural time and the registered turn.

use super::decision_admission::specification;
use super::decision_fixture::{fixture, ledger};
use crate::testing::declarations::TestResult;
use nrz_bindings::{
    BinaryOp, CheckedDeclarations, Context, CountDomain, DecisionExpression, Declaration,
    Descriptor, Dimension, Evidence, ExponentPolicy, IntegerDecisionCase, Kind, Measure,
    Observable, Registry, Role,
};
use nrz_cas::{ExactConstant, Rational};
use std::rc::Rc;

/// Exact nominal data and an explicit complete-turn period retain all original physical operands.
pub(super) fn physical_source(cycle_value: &str) -> TestResult<Rc<CheckedDeclarations>> {
    let time = Kind::Named(
        "duration".into(),
        Box::new(Dimension::integers([0, 0, 1, 0, 0, 0, 0])?),
    );
    let squared = Kind::Named(
        "squared_time".into(),
        Box::new(Dimension::integers([0, 0, 2, 0, 0, 0, 0])?),
    );
    let inductance = Kind::Named(
        "inductance".into(),
        Box::new(Dimension::integers([2, 1, -2, -2, 0, 0, 0])?),
    );
    let capacitance = Kind::Named(
        "capacitance".into(),
        Box::new(Dimension::integers([-2, -1, 4, 2, 0, 0, 0])?),
    );
    let mut registry = Registry::with_angle_frequency(ExponentPolicy::Rational)?;
    registry.register_binary(
        BinaryOp::Multiply,
        inductance.clone(),
        capacitance.clone(),
        squared.clone(),
    )?;
    registry.register_binary(BinaryOp::Multiply, time.clone(), time.clone(), squared)?;
    let cycle = registry.measure("turn").ok_or("missing full turn")?.clone();
    let radian = registry.measure("rad").ok_or("missing radian")?.clone();
    let time = Descriptor::new(time, Measure::coherent(), Role::Difference)?;
    let inductance = Descriptor::new(inductance, Measure::coherent(), Role::Difference)?;
    let capacitance = Descriptor::new(capacitance, Measure::coherent(), Role::Difference)?;
    let mut owner = Context::with_real_symbols(
        registry,
        nrz_bindings::Limits::default(),
        &[
            "L", "C", "tau", "duration", "cycle", "radian", "model", "period", "quotient",
        ],
    )?;
    for (name, descriptor, value) in [
        ("L", inductance, "1/40"),
        ("C", capacitance, "1/100000"),
        ("tau", time.clone(), "1/2000"),
        ("duration", time.clone(), "1/100"),
        ("cycle", cycle, cycle_value),
        ("radian", radian, "1"),
    ] {
        owner.declare(
            name.into(),
            Declaration::Constant {
                descriptor,
                value: Rational::parse(value, &mut ledger()?)?,
                evidence: Evidence {
                    source: "nominal source-free LC reference".into(),
                    assurance: "exact declared physical reference".into(),
                },
            },
        )?;
    }
    for (name, descriptor, expression) in [
        ("model", Descriptor::ratio(), "(tau*tau)/(L*C)"),
        ("period", time, "(cycle/radian)*tau"),
        ("quotient", Descriptor::ratio(), "duration/period"),
    ] {
        let expression = owner.parse(name, expression)?;
        owner.declare(
            name.into(),
            Declaration::Derived(Observable {
                descriptor,
                expression,
                evidence: Evidence {
                    source: "nominal LC model and declared epoch".into(),
                    assurance: "positive natural time; elapsed full periods from zero".into(),
                },
            }),
        )?;
    }
    Ok(Rc::new(owner.freeze()?))
}

/// The complete nominal model, registered full turn and original target give exactly three cycles.
/// ´test:integration:bindings-lc-count-complete´
#[test]
fn bindings_lc_count_complete() -> TestResult {
    let owner = physical_source("1")?;
    let mut budget = ledger()?;
    let model = DecisionExpression::declared(Rc::clone(&owner), "model", &mut budget)?;
    assert_eq!(model.exact(), &ExactConstant::one());
    let model_work = budget.usage().work;
    let physical = DecisionExpression::declared(owner, "quotient", &mut budget)?;
    let physical_work = budget.usage().work - model_work;
    let mut source = fixture(false, "3", "10/Pi")?;
    let case = IntegerDecisionCase::declare(
        &source.frame.cas,
        physical,
        specification(&source, CountDomain::NONNEGATIVE)?,
        &mut budget,
    )?;
    let witness = source
        .frame
        .cas
        .check_proof(&source.context, &source.steps, &mut budget)?;
    let graph = Rc::new(source.frame.cas);
    let before = graph.counts();
    let selected = case.admit(Rc::clone(&graph), witness, case.attachment(), &mut budget)?;
    let count = selected.count(3, &mut budget)?;
    assert_eq!(count.value(), 3);
    assert_eq!(
        count.descriptor().kind(),
        &Kind::Count("complete_lc_cycles".into(), CountDomain::NONNEGATIVE)
    );
    assert_eq!(graph.counts(), before);
    assert_eq!(model_work, 34_713);
    assert_eq!(physical_work, 81_363);
    assert_eq!(budget.usage().work, 315_878);
    Ok(())
}

/// Physical model and full-period quotient checks each retain their measured owner boundary.
/// ´test:integration:bindings-lc-count-physical-boundaries´
#[test]
fn bindings_lc_count_physical_boundaries() -> TestResult {
    let owner = physical_source("1")?;
    let before = owner.cas()?.counts();
    for (name, work, accepted) in [
        ("model", 34_713, true),
        ("model", 34_712, false),
        ("quotient", 81_363, true),
        ("quotient", 81_362, false),
    ] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        let result = DecisionExpression::declared(Rc::clone(&owner), name, &mut budget);
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(budget.usage().work, work);
        }
        assert_eq!(owner.cas()?.counts(), before);
    }
    Ok(())
}

/// The complete nominal LC lifecycle preserves prior spending and refuses one below the final count.
/// ´test:integration:bindings-lc-count-cumulative-boundary´
#[test]
fn bindings_lc_count_cumulative_boundary() -> TestResult {
    let owner = physical_source("1")?;
    for (work, accepted) in [(315_895, true), (315_894, false)] {
        let mut budget = nrz_bindings::Budget::new(nrz_bindings::Limits {
            work,
            ..nrz_bindings::Limits::default()
        })?;
        budget.work(17, nrz_cas::Location::new("prior nominal LC work"))?;
        let model = DecisionExpression::declared(Rc::clone(&owner), "model", &mut budget)?;
        assert_eq!(model.exact(), &ExactConstant::one());
        let physical = DecisionExpression::declared(Rc::clone(&owner), "quotient", &mut budget)?;
        let mut source = fixture(false, "3", "10/Pi")?;
        let case = IntegerDecisionCase::declare(
            &source.frame.cas,
            physical,
            specification(&source, CountDomain::NONNEGATIVE)?,
            &mut budget,
        )?;
        let witness = source
            .frame
            .cas
            .check_proof(&source.context, &source.steps, &mut budget)?;
        let selected = case.admit(
            Rc::new(source.frame.cas),
            witness,
            case.attachment(),
            &mut budget,
        )?;
        let before = selected.cas().counts();
        let result = selected.count(3, &mut budget);
        if accepted {
            assert_eq!(result?.value(), 3);
            assert_eq!(budget.usage().work, work);
        } else {
            let nrz_bindings::DecisionError::Algebra(fault) =
                result.err().ok_or("one-below LC accepted")?
            else {
                return Err("wrong LC refusal".into());
            };
            assert!(matches!(
                fault.kind,
                nrz_cas::FaultKind::Exhausted {
                    budget: nrz_cas::limits::BudgetKind::Work,
                    ..
                }
            ));
        }
        assert_eq!(
            selected.value(),
            &Rational::from_ratio(3, 1, &mut ledger()?)?
        );
        assert_eq!(selected.cas().counts(), before);
    }
    Ok(())
}

/// A half-turn period cannot be substituted into the original complete-cycle decision.
/// ´test:integration:bindings-lc-count-half-turn-refusal´
#[test]
fn bindings_lc_count_half_turn_refusal() -> TestResult {
    let physical =
        DecisionExpression::declared(physical_source("1/2")?, "quotient", &mut ledger()?)?;
    let source = fixture(false, "3", "10/Pi")?;
    let before = source.frame.cas.counts();
    assert!(matches!(
        IntegerDecisionCase::declare(
            &source.frame.cas,
            physical,
            specification(&source, CountDomain::NONNEGATIVE)?,
            &mut ledger()?
        ),
        Err(nrz_bindings::DecisionError::Target)
    ));
    assert_eq!(source.frame.cas.counts(), before);
    Ok(())
}
