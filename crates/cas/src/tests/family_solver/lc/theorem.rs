// SPDX-License-Identifier: AGPL-3.0-only
//! Complete original LC coverage retains exact construction and independent replay ledgers.

use crate::budget::BudgetUsage;
use crate::request::Outcome;
use crate::testing::common::TestResult;

fn generation(work: u64, arithmetic_steps: u64, denominator_factors: u64) -> BudgetUsage {
    BudgetUsage {
        work,
        arithmetic_steps,
        denominator_factors,
        case_arms: 1,
        coefficient_ops: 4_238,
        coefficient_maps: 4,
        tower_levels: 1,
        tower_degree: 2,
        gcd_steps: 45,
        selection_slots: 1,
        solution_branches: 2,
        ..BudgetUsage::default()
    }
}

fn replay() -> BudgetUsage {
    BudgetUsage {
        work: 1_813_435,
        arithmetic_steps: 29_506,
        condition_steps: 43,
        case_arms: 1,
        region_nodes: 5,
        coefficient_ops: 33_764,
        denominator_factors: 4,
        coefficient_maps: 4,
        tower_levels: 1,
        tower_degree: 2,
        tower_reductions: 1,
        gcd_steps: 72,
        selection_slots: 1,
        solution_branches: 1,
        solve_identities: 3,
        isolation_steps: 10,
        ..BudgetUsage::default()
    }
}

/// The original determinant succeeds without a reciprocal guard and replays from unproved syntax.
/// ´test:crate:lc-original-root-factor-theorem-replays´
#[test]
fn lc_original_root_factor_theorem_replays() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let outcome = source.cas.run(&request);
    let Outcome::Solved { answer, usage, .. } = outcome else {
        return Err(format!("LC producer: {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("LC coverage")?;
    crate::testing::solve_blind::family_lc(&source.cas, &coverage)?;
    let bytes = source
        .cas
        .export_witness(&coverage, &mut super::ledger()?)?;
    let mut checking = super::ledger()?;
    super::replay::fresh(&source.cas, &answer.set, &bytes, &mut checking)?;
    assert_eq!(usage, generation(838_465, 3_350, 4));
    assert_eq!(checking.usage(), replay());
    assert_eq!(bytes.len(), 6_140);
    Ok(())
}

/// Construction, independent checking and each fresh replayed rule have distinct pinned ledgers.
/// ´test:crate:lc-root-theorem-separate-ledgers´
#[test]
fn lc_root_theorem_separate_ledgers() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let mut proposal = super::ledger()?;
    let mut checking = super::ledger()?;
    let mut audit = crate::solving::radicals::route::Audit::default();
    let produced = crate::solving::solve_search::produce(
        &mut source.cas,
        &request,
        &mut audit,
        &mut proposal,
        &mut checking,
    )?;
    assert_eq!(produced.fault, None);
    let witness = produced.answer.coverage.as_ref().ok_or("LC coverage")?;
    let bytes = source.cas.export_witness(witness, &mut super::ledger()?)?;
    let mut imported = super::ledger()?;
    let ((), obligations) = crate::testing::replay_answer::measured(|| {
        super::replay::fresh(&source.cas, &produced.answer.set, &bytes, &mut imported)
    })?;
    let rules: Vec<_> = obligations
        .iter()
        .map(|row| {
            (
                row.rule.specification().key,
                row.after.work - row.before.work,
            )
        })
        .collect();
    assert_eq!(proposal.usage(), generation(301_736, 3_346, 2));
    assert_eq!(
        checking.usage(),
        BudgetUsage {
            work: 1_498_050,
            arithmetic_steps: 29_740,
            condition_steps: 53,
            coefficient_ops: 34_305,
            denominator_factors: 2,
            tower_reductions: 1,
            gcd_steps: 88,
            selection_slots: 1,
            solve_identities: 4,
            isolation_steps: 10,
            ..BudgetUsage::default()
        }
    );
    assert_eq!(imported.usage(), replay());
    assert_eq!(bytes.len(), 6_140);
    assert_eq!(
        rules,
        vec![
            ("real.sign", 1_430),
            ("extension.unit", 9_484),
            ("real.sign", 5_082),
            ("real.sign", 858),
            ("extension.unit", 9_490),
            ("extension.adjoin", 1_595),
            ("region.construct", 556),
            ("ctx.weaken", 3_354),
            ("ctx.weaken", 4_042),
            ("root.select", 431_249),
            ("coeff.identity", 51_019),
            ("solve.product", 202_440),
        ]
    );
    Ok(())
}
