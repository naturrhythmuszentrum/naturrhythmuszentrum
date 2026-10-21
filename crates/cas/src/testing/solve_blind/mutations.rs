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
                let original = certificate.factors.len();
                assert_eq!(original, coefficients.len() - 1);
                certificate.factors.pop().ok_or("missing factor")?;
                assert_ne!(certificate.factors.len(), original);
            }
            1 => {
                let factor = certificate.factors.first_mut().ok_or("missing factor")?;
                let original = factor.multiplicity;
                assert_eq!(original, 1);
                factor.multiplicity += 1;
                assert_ne!(factor.multiplicity, original);
            }
            _ => {
                let factor = certificate.factors.first_mut().ok_or("first factor")?;
                let original = factor.root.clone();
                assert_ne!(original, wrong_root);
                factor.root.clone_from(&wrong_root);
                assert_ne!(factor.root, original);
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

/// Establish the accepted sampled coverage before restoring missing-child mutation replay.
pub(crate) fn strata() -> TestResult {
    let keys = ["d", "c", "b", "a"];
    let (fixture, _, answer) = super::acceptance::solved(&keys)?;
    super::oracle::samples(
        &fixture.cas,
        answer.coverage.as_ref().ok_or("stratum coverage")?,
        &keys,
    )?;
    super::acceptance::fresh(&fixture.cas, &answer, &keys)
}
