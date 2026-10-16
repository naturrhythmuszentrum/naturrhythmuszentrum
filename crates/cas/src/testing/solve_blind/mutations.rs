// SPDX-License-Identifier: AGPL-3.0-only
//! Well-formed false certificates are checked against the actual blind result after search returns.

use crate::limits::{Budget, Limits};
use crate::proof_model::Certificate;
use crate::request::Outcome;
use crate::solving::SolveCertificate;
use crate::testing::common::TestResult;

/// Replacing or omitting a factor cannot borrow the original product identity to claim completeness.
pub(crate) fn factors(coefficients: &[i64]) -> TestResult {
    let (mut fixture, request) = super::source::polynomial(coefficients)?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Solved { answer, .. } = outcome else {
        return Err(format!("mutation baseline: {outcome:?}").into());
    };
    let coverage = answer.coverage.ok_or("mutation coverage")?;
    let steps = super::audit::applications(&fixture.cas, &coverage)?;
    let wrong_root = fixture.integer(0, false)?;
    for mutation in 0..3 {
        let mut changed = steps.clone();
        let root = changed.last_mut().ok_or("mutation root")?;
        let Certificate::Solve(certificate) = &mut root.certificate else {
            return Err("mutation certificate".into());
        };
        let SolveCertificate::ProductCompleteness(certificate) = certificate.as_mut() else {
            return Err("mutation product".into());
        };
        match mutation {
            0 => {
                certificate.factors.pop().ok_or("missing factor")?;
            }
            1 => {
                certificate
                    .factors
                    .first_mut()
                    .ok_or("missing factor")?
                    .multiplicity += 1;
            }
            _ => {
                certificate
                    .factors
                    .first_mut()
                    .ok_or("first factor")?
                    .root
                    .clone_from(&wrong_root);
            }
        }
        let error = fixture
            .cas
            .check_proof(
                &fixture.context,
                &changed,
                &mut Budget::new(Limits::blind_cubic())?,
            )
            .err()
            .ok_or("false product certificate accepted")?;
        assert!(
            matches!(error.kind, crate::error::FaultKind::Invalid(_)),
            "{error:?}"
        );
        assert_eq!(
            fixture
                .cas
                .export_witness(&coverage, &mut Budget::new(Limits::blind_cubic())?)?,
            coverage.bytes
        );
    }
    Ok(())
}

/// Pending coefficient-map interpretation design restores complete-cover stratum deletion mutations.
/// There is currently no complete symbolic cover to mutate, and its exact refusal stays executable.
pub(crate) fn strata() -> TestResult {
    let (mut fixture, request) = super::source::symbolic(&["d", "c", "b", "a"])?;
    let outcome = fixture.cas.run(&request);
    let Outcome::Invalid { fault, .. } = outcome else {
        return Err(format!("stratum boundary moved: {outcome:?}").into());
    };
    assert_eq!(
        fault.kind,
        crate::FaultKind::Invalid(crate::InvalidCode::UnsupportedFragment)
    );
    assert_eq!(fault.location.operation, "exact coefficients");
    Ok(())
}
