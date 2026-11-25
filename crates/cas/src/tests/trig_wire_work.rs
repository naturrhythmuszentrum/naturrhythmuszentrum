// SPDX-License-Identifier: AGPL-3.0-only
//! Encoding the complete proof closure is distinct from copying its sealed transport.

use crate::limits::BudgetKind;
use crate::syntax::Operator;
use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig::operators;
use crate::testing::trig_limits::{KINDS, boundary, limits};
use crate::testing::trig_replay::witness;
use crate::{FaultKind, Location};

fn attempt(operator: Operator, kind: BudgetKind, amount: u64) -> TestResult<(bool, u64)> {
    let (cas, proof) = witness(operator)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let expected = cas.export_witness(&proof, &mut default_budget()?)?;
    let ceilings = limits(kind, amount)?;
    let mut budget = crate::budget::Budget::new(ceilings.clone())?;
    budget.work(37, Location::new("prior trig encoding Work"))?;
    let result =
        budget.with_retained_payload(19, Location::new("prior trig encoding payload"), |budget| {
            crate::wire_build::encode(&cas, proof.proof.logical_id(), budget)
        });
    assert_eq!(budget.limits(), &ceilings);
    assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    match &result {
        Ok(bytes) => assert_eq!(bytes, &expected),
        Err(fault) => assert!(
            matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind),
            "{fault:?}"
        ),
    }
    assert!(budget.usage().work >= 37);
    assert!(budget.usage().work <= ceilings.work);
    Ok((result.is_ok(), budget.usage().work))
}

/// Work, depth, staged payload, payload, witness, output, source, occurrences, replay Work.
/// Work excludes 37 prior units; staged payload includes 19 retained caller bytes.
const COSTS: [u64; 9] = [19_045, 0, 14_547, 111, 661, 0, 0, 0, 0];

/// Every consumed encoder dimension has a literal success and one-below refusal.
/// ´test:crate:trig-encoding-boundaries´
#[test]
fn trig_encoding_boundaries() -> TestResult {
    for (operator, _) in operators() {
        for (kind, cost) in KINDS.into_iter().zip(COSTS) {
            boundary(kind, cost, |amount| attempt(operator.clone(), kind, amount))?;
        }
    }
    Ok(())
}
