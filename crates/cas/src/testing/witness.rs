// SPDX-License-Identifier: AGPL-3.0-only
//! Untrusted proof steps, completed outcomes and independent witness replay checks.
//!
//! Compiled only in the public test target.

use crate::test_api::{Cas, Handle, Limits, Outcome, Primitive, ProofStep, Relation, SearchResult};

use super::budgets::derivation_budget as budget;
use super::common::TestResult;
use super::requests::Fixture;

/// Only a completed successful outcome is acceptable for a positive fixture.
pub(crate) fn completed(outcome: Outcome) -> TestResult<SearchResult> {
    match outcome {
        Outcome::Verified { result, .. } | Outcome::Conditional { result, .. } => Ok(result),
        other => Err(format!("expected completed derivation, got {other:?}").into()),
    }
}

/// One untrusted exact equality proposal, used for independently supplied target formulas.
pub(crate) fn equality(primitive: Primitive, left: &Handle, right: &Handle) -> ProofStep {
    ProofStep {
        primitive,
        relation: Relation::EqualOnDomain,
        left: left.clone(),
        right: right.clone(),
        premises: Vec::new(),
        path: Vec::new(),
        substitutions: Vec::new(),
        certificate: None,
    }
}

/// A derivative application with explicit ordered premise references.
pub(crate) fn derivative(
    primitive: Primitive,
    left: &Handle,
    right: &Handle,
    premises: &[u32],
) -> ProofStep {
    ProofStep {
        primitive,
        relation: Relation::Derivative {
            variable: "x".into(),
        },
        left: left.clone(),
        right: right.clone(),
        premises: premises.to_vec(),
        path: Vec::new(),
        substitutions: Vec::new(),
        certificate: None,
    }
}

/// Check a produced expression against an independently written exact formula.
pub(crate) fn equal(fixture: &mut Fixture, left: &Handle, right: &Handle) -> TestResult {
    fixture.cas.check_proof(
        &fixture.context,
        &[equality(Primitive::RationalIdentity, left, right)],
        &mut budget()?,
    )?;
    Ok(())
}

/// Full proof replay works in an empty store without the generator's memo table.
pub(crate) fn replay(fixture: &Fixture, result: &SearchResult) -> TestResult<Vec<u8>> {
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut fresh = Cas::new(Limits::default())?;
    let checked = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.export_witness(&checked, &mut budget()?)?, bytes);
    Ok(bytes)
}
