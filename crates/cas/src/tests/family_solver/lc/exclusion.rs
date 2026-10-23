// SPDX-License-Identifier: AGPL-3.0-only
//! Exact empty zero-set evidence is shared without discarding original domains or context.

use crate::proof::Certificate;
use crate::request::Outcome;
use crate::testing::common::TestResult;

/// Anonymous sign leaves retain the original witness bytes without synthetic exported zero loci.
/// ´test:crate:lc-empty-lemmas-retain-exact-subjects´
#[test]
fn lc_empty_lemmas_retain_exact_subjects() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("LC coverage".into());
    };
    let witness = answer.coverage.as_ref().ok_or("LC coverage")?;
    let steps = crate::testing::solve_blind::family_applications(&source.cas, witness)?;
    assert_eq!(
        steps
            .iter()
            .filter(|step| matches!(&step.certificate, Certificate::Emptiness(_)))
            .count(),
        0
    );
    crate::testing::solve_blind::family_lc(&source.cas, witness)?;
    assert_eq!(
        source
            .cas
            .export_witness(witness, &mut super::ledger()?)?
            .len(),
        6_140
    );
    Ok(())
}

/// A nonzero product supplies its factors, while an uncovered variable or zero scalar is refused.
/// ´test:crate:empty-monomial-factor-inference´
#[test]
fn empty_monomial_factor_inference() -> TestResult {
    use crate::solving::coefficients::{arithmetic::Engine, forms::Variable};
    use crate::solving::elementary::{exclusion, value::Value};
    let (source, _) = crate::testing::solve_blind::symbolic(&["p"])?;
    let engine = Engine {
        state: &source.cas.state,
        domain: &source.algebra,
    };
    let mut budget = super::ledger()?;
    let first = Value::variable(Variable::Slot(0), &engine, &mut budget)?;
    let second = Value::variable(Variable::Slot(1), &engine, &mut budget)?;
    let unknown = Value::variable(Variable::Slot(2), &engine, &mut budget)?;
    let product = first.mul(&second, &engine, &mut budget)?;
    let query = first.pow(2, &engine, &mut budget)?.mul(
        &second.pow(3, &engine, &mut budget)?,
        &engine,
        &mut budget,
    )?;
    let facts = std::slice::from_ref(&product);
    assert!(exclusion::monomial(
        &query.numerator,
        &facts.iter(),
        &mut budget
    )?);
    assert!(!exclusion::monomial(
        &unknown.numerator,
        &facts.iter(),
        &mut budget
    )?);
    let zero = Value::integer(0, &engine, &mut budget)?;
    assert!(!exclusion::monomial(
        &zero.numerator,
        &facts.iter(),
        &mut budget
    )?);
    assert!(!exclusion::same_form(
        &first.numerator,
        &second.numerator,
        &mut budget
    )?);
    assert!(exclusion::same_form(
        &query.numerator,
        &query.numerator,
        &mut budget
    )?);
    Ok(())
}

/// Every new allocation-free leaf operation charges its exact cumulative Work boundary.
/// ´test:crate:empty-inline-leaf-work-boundaries´
#[test]
fn empty_inline_leaf_work_boundaries() -> TestResult {
    use crate::solving::coefficients::{arithmetic::Engine, forms::Variable};
    use crate::solving::elementary::{exclusion, value::Value};
    let (source, _) = crate::testing::solve_blind::symbolic(&["p"])?;
    let engine = Engine {
        state: &source.cas.state,
        domain: &source.algebra,
    };
    let first = Value::variable(Variable::Slot(0), &engine, &mut super::ledger()?)?;
    let facts = std::slice::from_ref(&first);
    leaf_boundary(|budget| exclusion::same_form(&first.numerator, &first.numerator, budget))?;
    leaf_boundary(|budget| exclusion::matches(&first, facts, budget))?;
    leaf_boundary(|budget| exclusion::monomial(&first.numerator, &facts.iter(), budget))?;
    Ok(())
}

fn leaf_boundary(
    mut check: impl FnMut(&mut crate::budget::Budget) -> Result<bool, crate::Fault>,
) -> TestResult {
    let mut ledger = super::ledger()?;
    assert!(check(&mut ledger)?);
    let required = ledger.usage().work;
    assert!(required > 0);
    for short in [0, 1] {
        let mut limits = ledger.limits().clone();
        limits.work = required - short;
        let mut budget = crate::budget::Budget::new(limits)?;
        let result = check(&mut budget);
        if short == 0 {
            assert!(result?);
            assert_eq!(budget.usage().work, required);
        } else {
            assert!(matches!(
                result.err().ok_or("short leaf budget accepted")?.kind,
                crate::FaultKind::Exhausted {
                    budget: crate::budget::BudgetKind::Work,
                    ..
                }
            ));
        }
    }
    Ok(())
}
