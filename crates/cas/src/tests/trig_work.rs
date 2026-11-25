// SPDX-License-Identifier: AGPL-3.0-only
//! Complete canonical-index lookup Work is independent of hit or miss outcome.

use crate::graph::{LogicalId, Vertex, vertex_key};
use crate::limits::{BudgetKind, Limits};
use crate::syntax::Operator;
use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig::{Fixture, operators};
use crate::{FaultKind, Location};

fn fixture(
    operator: Operator,
    projected: bool,
    hit: bool,
) -> TestResult<(Fixture, Vec<u8>, Option<LogicalId>)> {
    let mut budget = crate::testing::common::default_budget()?;
    let mut f = Fixture::new()?;
    let operand = if projected {
        f.term(Operator::RealPart, &[f.z.clone()])?
    } else {
        f.x.clone()
    };
    let stored = if hit { operator.clone() } else { Operator::Neg };
    let node = f.term(stored, &[operand])?;
    let mut vertex = f.cas.state.node(node.logical_id(), &mut budget)?.clone();
    let Vertex::Expression(expression) = &mut vertex else {
        return Err("expression".into());
    };
    expression.operator = operator;
    let key = vertex_key(&vertex, &f.cas.state, &mut default_budget()?)?.1;
    Ok((f, key, hit.then_some(node.logical_id())))
}

fn boundary(
    operator: Operator,
    projected: bool,
    hit: bool,
    work: u64,
    succeeds: bool,
) -> TestResult {
    let (f, key, expected) = fixture(operator, projected, hit)?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = f.cas.counts();
    let comparisons = f.cas.interning_snapshot();
    let mut budget = crate::budget::Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(37, Location::new("prior trig interning Work"))?;
    let result = f.cas.state.interned(&key, &mut budget);
    if succeeds {
        assert_eq!(result?, expected);
        assert_eq!(budget.usage().work, work);
    } else {
        let fault = result.err().ok_or("lookup exceeded Work")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert_eq!(budget.usage().work, 37);
        assert_eq!(f.cas.interning_snapshot(), comparisons);
    }
    assert_eq!(budget.limits().work, work);
    assert_eq!(f.cas.counts(), counts);
    assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    Ok(())
}

/// Literal hit and miss boundaries preserve prior Work and the complete graph on exhaustion.
/// ´test:crate:trig-interning-work´
#[test]
fn trig_interning_work() -> TestResult {
    for (operator, _) in operators() {
        for projected in [false, true] {
            for hit in [false, true] {
                boundary(operator.clone(), projected, hit, 51 + 36, false)?;
                boundary(operator.clone(), projected, hit, 51 + 37, true)?;
            }
        }
    }
    Ok(())
}
