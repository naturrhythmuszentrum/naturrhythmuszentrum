// SPDX-License-Identifier: AGPL-3.0-only
//! Independent inverse declaration renaming compares sampled equations and complete strata.

use crate::testing::common::TestResult;

/// Bind both original declaration frames to the same ordered values and compare their leaf inventories.
pub(crate) fn complete() -> TestResult {
    let mut inventories = Vec::new();
    let mut guards = Vec::new();
    for keys in [["d", "c", "b", "a"], ["delta", "gamma", "beta", "alpha"]] {
        let (fixture, request, answer) = super::acceptance::solved(&keys)?;
        super::acceptance::fresh(&fixture.cas, &answer, &keys)?;
        let crate::request::Task::Solve(specification) = &request.task else {
            return Err("renaming request".into());
        };
        let original =
            crate::solving::elementary::proof::problem(&fixture.cas, &specification.problem)?;
        let frame =
            crate::solving::elementary::proof::coordinates(&fixture.cas, &original.coefficients)?;
        assert!(
            frame
                .coordinates
                .iter()
                .all(|coordinate| coordinate.field == crate::syntax::Field::Complex)
        );
        let declared: Vec<_> = frame
            .coordinates
            .iter()
            .map(|coordinate| coordinate.key.as_str())
            .collect();
        assert_eq!(declared, keys);
        let coverage = answer.coverage.as_ref().ok_or("renamed coverage")?;
        inventories.push(super::oracle::samples(&fixture.cas, coverage, &declared)?);
        guards.push(super::oracle::guards(&fixture.cas, coverage, &declared)?);
    }
    assert_eq!(inventories.first(), inventories.get(1));
    assert_eq!(guards.first(), guards.get(1));
    Ok(())
}
