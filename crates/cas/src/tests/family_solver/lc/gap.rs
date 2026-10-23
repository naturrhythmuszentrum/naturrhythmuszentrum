// SPDX-License-Identifier: AGPL-3.0-only
//! The original reciprocal-square-root request retains its precise remaining producer boundary.

use crate::budget::BudgetUsage;
use crate::request::Outcome;
use crate::testing::common::TestResult;
use crate::{Fault, InvalidCode, Location};

/// Checked signs and coefficient transport reach a remaining elementary reciprocal-root refusal.
/// ´test:crate:lc-reciprocal-root-refinement-producer-gap´
#[test]
fn lc_reciprocal_root_refinement_producer_gap() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let outcome = source.cas.run(&request);
    let Outcome::Invalid { fault, usage } = outcome else {
        return Err(format!("LC producer boundary changed: {outcome:?}").into());
    };
    assert_eq!(
        fault,
        Fault::invalid(InvalidCode::Rule, Location::new("elementary solving"))
    );
    assert_eq!(
        usage,
        BudgetUsage {
            work: 838_426,
            arithmetic_steps: 3_350,
            case_arms: 1,
            coefficient_ops: 4_238,
            denominator_factors: 4,
            coefficient_maps: 4,
            tower_levels: 1,
            tower_degree: 2,
            gcd_steps: 45,
            selection_slots: 1,
            solution_branches: 2,
            ..BudgetUsage::default()
        }
    );
    Ok(())
}
