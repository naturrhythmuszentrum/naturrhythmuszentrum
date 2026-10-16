// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit admission boundary for the only proof route supporting bound context predicates.

use super::storage::{AT, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Scope;
use crate::proof_model::{ApplicationDraft, Certificate, CheckedApplication, Statement};
use crate::rule::Primitive;
use crate::solving::{SolveCertificate, SolveSchema, SolveStatement};

/// Every mathematical object of a supporting theorem is independent of the formal-root binder.
fn objects<'a>(
    cas: &Cas,
    values: impl IntoIterator<Item = &'a Handle>,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for handle in values {
        super::independent::object(cas, handle, scope, budget)?;
    }
    Ok(())
}

/// Supporting units and existence cannot consume a root-dependent hypothesis or residual.
fn premises(
    cas: &Cas,
    earlier: &[CheckedApplication],
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in earlier {
        for reference in &premise.conditions.hypotheses {
            budget.work(1, AT)?;
            let frame = cas.state.context(cas.checked(&reference.context)?)?;
            let handle = if reference.slot == 0 && frame.parent.is_some() {
                cas.handle(frame.admissible)
            } else {
                crate::context_check::predicate(cas, reference)?
            };
            super::independent::object(cas, &handle, scope, budget)?;
        }
        for condition in premise
            .conditions
            .requirements
            .iter()
            .chain(&premise.conditions.residuals)
        {
            objects(
                cas,
                [&condition.region, &condition.predicate],
                scope,
                budget,
            )?;
        }
    }
    Ok(())
}

/// Whitelisted support retains the original checks after proving root independence of every object.
fn support(
    cas: &Cas,
    draft: &ApplicationDraft,
    scope: &Scope,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let (Certificate::Solve(certificate), Statement::Solve(statement)) =
        (&draft.certificate, &draft.conclusion)
    else {
        return Err(invalid());
    };
    match (certificate.as_ref(), statement.as_ref()) {
        (SolveCertificate::RootAdjunction(value), SolveStatement::RootExistence(claim)) => objects(
            cas,
            [&claim.relation, &claim.region, &value.relation]
                .into_iter()
                .chain(value.selection.iter())
                .chain(&value.splitting_chain),
            scope,
            budget,
        ),
        (SolveCertificate::ExtensionUnit(value), SolveStatement::Unit(claim)) => objects(
            cas,
            [
                &claim.value,
                &claim.inverse,
                &claim.domain,
                &claim.region,
                &value.element,
                &value.modulus,
                &value.gcd,
                &value.region,
            ]
            .into_iter()
            .chain(&value.quotients)
            .chain(&value.bezout),
            scope,
            budget,
        ),
        _ => Err(invalid()),
    }
}

/// All other native consumers retain their former closed-context precondition explicitly.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    earlier: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let frame = cas.state.context(cas.checked(&draft.context)?)?;
    let scope = &cas.state.region(frame.admissible)?.scope;
    if scope.is_empty() {
        for premise in earlier {
            budget.work(1, AT)?;
            crate::context::require_closed(&cas.state, cas.checked(&premise.judgment.context)?)?;
        }
        return Ok(());
    }
    match draft.rule {
        Primitive::ContextHypothesis | Primitive::RegionConstruct | Primitive::ContextClose => {
            Ok(())
        }
        Primitive::Solve(SolveSchema::CoefficientIdentity) => {
            if let Certificate::Solve(certificate) = &draft.certificate
                && let SolveCertificate::CoefficientIdentity(identity) = certificate.as_ref()
                && identity
                    .combinations
                    .iter()
                    .any(|row| row.hypothesis.is_some())
            {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        Primitive::Solve(SolveSchema::RootAdjunction | SolveSchema::ExtensionUnit) => {
            support(cas, draft, scope, budget)?;
            premises(cas, earlier, scope, budget)
        }
        _ => Err(invalid()),
    }
}
