// SPDX-License-Identifier: AGPL-3.0-only
//! Coefficient renaming preserves the explicit pending symbolic-map boundary.

use crate::testing::common::TestResult;

/// Pending coefficient-map interpretation design restores transport of complete renamed sets.
pub(crate) fn complete() -> TestResult {
    for names in [["d", "c", "b", "a"], ["delta", "gamma", "beta", "alpha"]] {
        let (mut fixture, request) = super::source::symbolic(&names)?;
        let outcome = fixture.cas.run(&request);
        let crate::request::Outcome::Invalid { fault, .. } = outcome else {
            return Err(format!("renaming boundary moved: {outcome:?}").into());
        };
        assert_eq!(
            fault.kind,
            crate::FaultKind::Invalid(crate::InvalidCode::UnsupportedFragment)
        );
        assert_eq!(fault.location.operation, "exact coefficients");
    }
    Ok(())
}
