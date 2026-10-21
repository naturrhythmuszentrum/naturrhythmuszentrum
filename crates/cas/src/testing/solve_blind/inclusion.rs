// SPDX-License-Identifier: AGPL-3.0-only
//! Optional factor comparisons cannot turn a refused equality into an inclusion theorem.

use crate::graph::{Cas, LogicalId};
use crate::limits::{Budget, Limits};
use crate::proof_model::{Certificate, CheckedApplication, Statement, VerifiedWitness};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::elementary::{evaluate::Evaluation, product};
use crate::solving::{CertificateProductCompleteness, SolveCertificate, SolveStatement};
use crate::testing::common::TestResult;
use std::collections::BTreeSet;

/// Inspect only the accepted proof's authoritative dependency closure.
fn products(cas: &Cas, coverage: &VerifiedWitness) -> TestResult<Vec<CheckedApplication>> {
    let mut pending = vec![coverage.proof().logical_id()];
    let mut seen = BTreeSet::<LogicalId>::new();
    let mut result = Vec::new();
    let mut budget = Budget::new(Limits::blind_cubic())?;
    while let Some(proof) = pending.pop() {
        if !seen.insert(proof) {
            continue;
        }
        let application = crate::proof_view::application(cas, proof, &mut budget)?;
        pending.extend(&application.premises);
        if matches!(&application.certificate, Certificate::Solve(value)
            if matches!(value.as_ref(), SolveCertificate::ProductCompleteness(value) if value.factors.len() == 3))
        {
            result.push(application);
        }
    }
    Ok(result)
}

/// Each distinct represented root matches its own entry; different entries are refused candidates.
fn comparisons(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateProductCompleteness,
    entries: &[crate::Handle],
) -> TestResult {
    assert_eq!(certificate.factors.len(), 3);
    assert_eq!(entries.len(), 3);
    for (position, factor) in certificate.factors.iter().enumerate() {
        assert_eq!(factor.root, entries[position]);
        assert_eq!(factor.multiplicity, 1);
        for (index, entry) in entries.iter().enumerate() {
            let mut budget = Budget::new(Limits::blind_cubic())?;
            let root = evaluation.expression(factor.root.logical_id(), true, &mut budget)?;
            let candidate = evaluation.expression(entry.logical_id(), true, &mut budget)?;
            let result = evaluation.zero(
                &root.sub(&candidate, &evaluation.engine, &mut budget)?,
                &mut budget,
            );
            if position == index {
                result?;
            } else {
                let error = result.err().ok_or("distinct candidate was accepted")?;
                assert!(matches!(
                    error.kind,
                    crate::FaultKind::Invalid(crate::InvalidCode::Rule)
                ));
                assert!(!product::decides(Err(error))?);
            }
        }
    }
    Ok(())
}

/// Reordering searches past refused candidates; omitting the actual matches cannot supply completeness.
fn application(cas: &Cas, checked: &CheckedApplication) -> TestResult {
    let Certificate::Solve(certificate) = &checked.certificate else {
        return Err("product certificate".into());
    };
    let SolveCertificate::ProductCompleteness(certificate) = certificate.as_ref() else {
        return Err("product payload".into());
    };
    let Statement::Solve(statement) = &checked.judgment.statement else {
        return Err("product statement".into());
    };
    let SolveStatement::Coverage(coverage) = statement.as_ref() else {
        return Err("product coverage".into());
    };
    let original = crate::solving::elementary::proof::problem(cas, &certificate.problem)?;
    assert_eq!(original.denominators, [] as [crate::Handle; 0]);
    let mut budget = Budget::new(Limits::blind_cubic())?;
    let mut premises = Vec::new();
    for proof in &checked.premises {
        premises.push(crate::proof_view::application(cas, *proof, &mut budget)?);
    }
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &certificate.region,
        premises: &premises,
        free: &[],
        bound: &[],
    };
    let mut entries = product::entries(cas, coverage, &mut budget)?;
    comparisons(&evaluation, certificate, &entries)?;
    product::factors(
        &evaluation,
        original,
        certificate,
        &entries,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    let original_order = entries.clone();
    entries.reverse();
    assert_ne!(entries, original_order);
    product::factors(
        &evaluation,
        original,
        certificate,
        &entries,
        &mut Budget::new(Limits::blind_cubic())?,
    )?;
    assert_eq!(entries.len(), 3);
    entries.truncate(1);
    assert_ne!(entries, original_order);
    let error = product::factors(
        &evaluation,
        original,
        certificate,
        &entries,
        &mut Budget::new(Limits::blind_cubic())?,
    )
    .err()
    .ok_or("unmatched factor accepted")?;
    assert!(matches!(
        error.kind,
        crate::FaultKind::Invalid(crate::InvalidCode::Rule)
    ));
    Ok(())
}

/// The accepted cubic application in the measured final stratum retains every exact factor match.
pub(crate) fn check() -> TestResult {
    let keys = ["d", "c", "b", "a"];
    let (fixture, _, answer) = super::acceptance::solved(&keys)?;
    let coverage = answer.coverage.as_ref().ok_or("inclusion coverage")?;
    let candidates = products(&fixture.cas, coverage)?;
    let measured = candidates.iter().find(|checked| matches!(&checked.certificate, Certificate::Solve(value)
        if matches!(value.as_ref(), SolveCertificate::ProductCompleteness(value) if value.region.logical_id().serial == 42)))
        .ok_or("measured Region 42 product application missing")?;
    application(&fixture.cas, measured)?;
    super::acceptance::fresh(&fixture.cas, &answer, &keys)
}
