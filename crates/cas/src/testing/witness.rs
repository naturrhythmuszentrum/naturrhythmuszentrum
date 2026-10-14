// SPDX-License-Identifier: AGPL-3.0-only
//! Typed scalar proposals, completed outcomes and independent witness replay checks.

use crate::test_api::domain::Layer;
use crate::test_api::proof::{
    ApplicationDraft, Certificate, DomainContract, GuardedObligation, Judgment, Primitive,
    Statement,
};
use crate::test_api::request::{Outcome, SearchResult};
use crate::test_api::{Cas, Handle};

use super::budgets::derivation_budget as budget;
use super::common::TestResult;
use super::requests::Fixture;

/// Only a completed successful outcome is acceptable for a positive fixture.
pub(crate) fn completed(outcome: Outcome) -> TestResult<SearchResult> {
    match outcome {
        Outcome::Proved { result, .. } | Outcome::Conditional { result, .. } => Ok(result),
        other => Err(format!("expected completed derivation, got {other:?}").into()),
    }
}

/// One untrusted equality proposal with the full context and domain made explicit.
pub(crate) fn equality(
    context: &Handle,
    domain: &Handle,
    rule: Primitive,
    left: &Handle,
    right: &Handle,
) -> ApplicationDraft {
    ApplicationDraft {
        rule,
        context: context.clone(),
        conclusion: Statement::Equality {
            left: left.clone(),
            right: right.clone(),
            domain: domain.clone(),
            domain_contract: DomainContract::DefinedAndEqual,
        },
        premises: Vec::new(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    }
}

/// A typed derivative proposal with explicit domain and ordered premises.
pub(crate) fn derivative(
    context: &Handle,
    domain: &Handle,
    rule: Primitive,
    left: &Handle,
    right: &Handle,
    premises: &[u32],
) -> ApplicationDraft {
    ApplicationDraft {
        rule,
        context: context.clone(),
        conclusion: Statement::DerivativeOn {
            function: left.clone(),
            derivative: right.clone(),
            coordinate: "x".into(),
            domain: domain.clone(),
        },
        premises: premises.to_vec(),
        substitutions: Vec::new(),
        certificate: Certificate::None,
    }
}

/// Inspect only statements with a declared scalar input; a region fact has no dummy endpoint.
pub(crate) fn left(judgment: &Judgment) -> TestResult<&Handle> {
    match &judgment.statement {
        Statement::Equality { left, .. } => Ok(left),
        Statement::DerivativeOn { function, .. } => Ok(function),
        Statement::Jacobian { input, .. } | Statement::SquareFree { input, .. } => Ok(input),
        _ => Err("expected a scalar input statement".into()),
    }
}

/// Inspect a typed scalar output without conflating it with a cover or predicate theorem.
pub(crate) fn right(judgment: &Judgment) -> TestResult<&Handle> {
    match &judgment.statement {
        Statement::Equality { right, .. } => Ok(right),
        Statement::DerivativeOn { derivative, .. } => Ok(derivative),
        Statement::Jacobian { output, .. } | Statement::SquareFree { output, .. } => Ok(output),
        _ => Err("expected a scalar output statement".into()),
    }
}

/// Independently construct every field of a required scalar predicate tuple.
pub(crate) fn guarded(
    cas: &mut Cas,
    context: &Handle,
    kind: crate::test_api::proof::GuardKind,
    term: &Handle,
) -> TestResult<GuardedObligation> {
    use crate::test_api::syntax::{Draft, PredicateKind};
    let header = cas.expression_header(term)?;
    let scope = cas.expression_scope(term)?.clone();
    let kind = match kind {
        crate::test_api::proof::GuardKind::Defined => PredicateKind::Defined,
        crate::test_api::proof::GuardKind::NonZero => PredicateKind::NonZero,
    };
    let predicate = super::builders::admit(
        cas,
        Draft::Condition {
            kind,
            header: crate::test_api::Reference::Existing(header),
            scope: scope.clone(),
            arguments: vec![crate::test_api::Reference::Existing(term.clone())],
        },
        &mut budget()?,
    )?;
    Ok(GuardedObligation {
        home: context.clone(),
        scope,
        layer: Layer::Pointwise,
        space: None,
        region: cas.admissible_region(context)?,
        predicate,
    })
}

/// Check a produced expression against an independently written exact formula.
pub(crate) fn equal(fixture: &mut Fixture, left: &Handle, right: &Handle) -> TestResult {
    fixture.cas.check_proof(
        &fixture.context,
        &[equality(
            &fixture.context,
            &fixture.domain,
            Primitive::RationalIdentity,
            left,
            right,
        )],
        &mut budget()?,
    )?;
    Ok(())
}

/// Replay in an empty store without the generator's memo table.
pub(crate) fn replay(fixture: &Fixture, result: &SearchResult) -> TestResult<Vec<u8>> {
    let bytes = fixture
        .cas
        .export_witness(&result.witness, &mut budget()?)?;
    let mut fresh = Cas::new(budget()?.limits().clone())?;
    let checked = fresh.inspect(&bytes, &mut budget()?)?;
    assert_eq!(fresh.export_witness(&checked, &mut budget()?)?, bytes);
    Ok(bytes)
}
