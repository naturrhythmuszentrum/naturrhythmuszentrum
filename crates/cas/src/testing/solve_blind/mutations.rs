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

/// Each declared zero/nonzero child is necessary in an independently checked complete coverage DAG.
pub(crate) fn strata() -> TestResult {
    use crate::solving::PredicateSplitMode;
    let (mut fixture, _, answer) = super::acceptance::solved(&["d", "c", "b", "a"])?;
    let coverage = answer.coverage.as_ref().ok_or("stratum coverage")?;
    super::oracle::samples(&fixture.cas, coverage, &["d", "c", "b", "a"])?;
    let steps = super::audit::applications(&fixture.cas, coverage)?;
    let mut count = 0;
    for (index, step) in steps.iter().enumerate() {
        let Certificate::Solve(certificate) = &step.certificate else {
            continue;
        };
        let SolveCertificate::TotalPredicateSplit(split) = certificate.as_ref() else {
            continue;
        };
        if split.mode != PredicateSplitMode::EliminateCover {
            continue;
        }
        assert_eq!(split.children.len(), 2);
        assert_eq!(split.branches.len(), 2);
        for position in 0..2 {
            let mut changed = steps.clone();
            let Certificate::Solve(certificate) =
                &mut changed.get_mut(index).ok_or("stratum step")?.certificate
            else {
                return Err("stratum certificate".into());
            };
            let SolveCertificate::TotalPredicateSplit(split) = certificate.as_mut() else {
                return Err("stratum split".into());
            };
            let removed = split.children.remove(position);
            split.branches.remove(position);
            assert_eq!(split.children.len(), 1);
            assert_ne!(split.children.first(), Some(&removed));
            let error = fixture
                .cas
                .check_proof(
                    &fixture.context,
                    &changed,
                    &mut Budget::new(Limits::blind_cubic())?,
                )
                .err()
                .ok_or("missing stratum accepted")?;
            assert!(
                matches!(error.kind, crate::FaultKind::Invalid(_)),
                "{error:?}"
            );
            count += 1;
        }
    }
    assert!(count >= 8);
    super::acceptance::fresh(&fixture.cas, &answer, &["d", "c", "b", "a"])
}
