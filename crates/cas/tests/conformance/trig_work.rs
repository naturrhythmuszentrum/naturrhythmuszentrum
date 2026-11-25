// SPDX-License-Identifier: AGPL-3.0-only
//! Cumulative admission Work for cold and interned closed Real constructors.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig::{Fixture, operators};
use nrz_cas::limits::{BudgetKind, Limits};
use nrz_cas::syntax::{Draft, Operator};
use nrz_cas::{FaultKind, Location};

fn fixture(operator: Operator, projected: bool, warm: bool) -> TestResult<(Fixture, Draft)> {
    let mut f = Fixture::new()?;
    let operand = if projected {
        f.term(Operator::RealPart, &[f.z.clone()])?
    } else {
        f.x.clone()
    };
    let draft = Draft::Expression(f.draft(operator.clone(), std::slice::from_ref(&operand)));
    if warm {
        f.term(operator, &[operand])?;
    }
    Ok((f, draft))
}

fn boundary(
    operator: Operator,
    projected: bool,
    warm: bool,
    work: u64,
    succeeds: bool,
) -> TestResult {
    let (mut f, draft) = fixture(operator, projected, warm)?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = f.cas.counts();
    let mut budget = nrz_cas::limits::Budget::new(Limits {
        work,
        ..Limits::default()
    })?;
    budget.work(37, Location::new("prior trig admission Work"))?;
    let result = f.cas.admit(&[draft], &mut budget);
    assert!(budget.usage().work > 37);
    if succeeds {
        assert_eq!(result?.len(), 1);
        assert_eq!(budget.usage().work, work);
    } else {
        let fault = result.err().ok_or("admission exceeded Work")?;
        assert!(matches!(
            fault.kind,
            FaultKind::Exhausted {
                budget: BudgetKind::Work,
                ..
            }
        ));
        assert!(budget.usage().work <= work);
    }
    assert_eq!(budget.limits().work, work);
    if !succeeds || warm {
        assert_eq!(f.cas.counts(), counts);
        assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok(())
}

/// Literal N/N−1 pins include prior caller Work for cold and warm Real admission paths.
/// ´test:integration:trig-admission-work´
#[test]
fn trig_admission_work() -> TestResult {
    for (operator, _) in operators() {
        for projected in [false, true] {
            for (warm, work) in [(false, 1_035), (true, 928)] {
                boundary(operator.clone(), projected, warm, work - 1, false)?;
                boundary(operator.clone(), projected, warm, work, true)?;
            }
        }
    }
    Ok(())
}
