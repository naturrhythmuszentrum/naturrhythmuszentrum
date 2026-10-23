// SPDX-License-Identifier: AGPL-3.0-only
//! Attribution preserves cumulative refusal boundaries and restores nested accounting scopes.

use crate::budget::{Budget, Limits};
use crate::error::{Fault, Location};
use crate::testing::common::TestResult;
use crate::testing::work_trace::{Row, Scope, measured};

/// A failing nested operation retains only successfully charged Work and restores its caller scope.
fn refused(budget: &mut Budget) -> Result<(), Fault> {
    let _scope = Scope::phase("excluded-zero");
    budget.work(2, Location::new("inner"))?;
    budget.work(1, Location::new("unperformed"))
}

/// Observation excludes prior consumption and counts no work for an exhausted operation.
/// ´test:crate:work-attribution-preserves-refusal-boundary´
#[test]
fn work_attribution_preserves_refusal_boundary() -> TestResult {
    let mut budget = Budget::new(Limits {
        work: 5,
        ..Limits::default()
    })?;
    budget.work(3, Location::new("prior"))?;
    let (result, rows) = measured(|| {
        let _scope = Scope::rule("test.root");
        refused(&mut budget)
    })?;
    assert!(result.is_err());
    assert_eq!(budget.usage().work, 5);
    assert_eq!(
        rows,
        [Row {
            rule: "test.root",
            phase: "excluded-zero",
            location: "inner",
            work: 2,
        }]
    );
    let mut budget = Budget::new(Limits {
        work: 4,
        ..Limits::default()
    })?;
    budget.work(3, Location::new("prior"))?;
    let (result, rows) = measured(|| refused(&mut budget))?;
    assert!(result.is_err());
    assert_eq!(budget.usage().work, 3);
    assert_eq!(rows, [] as [Row; 0]);
    Ok(())
}

/// Nested measurements refuse without replacing the active rows, and later measurements start empty.
/// ´test:crate:work-attribution-restores-observation´
#[test]
fn work_attribution_restores_observation() -> TestResult {
    let mut budget = Budget::new(Limits::default())?;
    let (result, rows) = measured(|| -> TestResult {
        let _scope = Scope::rule("test.root");
        assert!(measured(|| ()).is_err());
        budget.work(1, Location::new("outer"))?;
        Ok(())
    })?;
    result?;
    assert_eq!(
        rows,
        [Row {
            rule: "test.root",
            phase: "validation",
            location: "outer",
            work: 1,
        }]
    );
    let (result, rows) = measured(|| budget.work(2, Location::new("later")))?;
    result?;
    assert_eq!(
        rows,
        [Row {
            rule: "replay",
            phase: "overhead",
            location: "later",
            work: 2,
        }]
    );
    assert_eq!(budget.usage().work, 3);
    Ok(())
}
