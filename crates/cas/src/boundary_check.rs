// SPDX-License-Identifier: AGPL-3.0-only
//! Closed boundary schemas preserve every consumed hypothesis and residual.

use crate::boundary_model::BoundaryCertificate;
use crate::budget::Budget;
use crate::case_syntax::invalid;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::proof_model::{
    ApplicationDraft, Certificate, CheckedApplication, DomainContract, ScopedConditions, Statement,
};
use crate::rule::Primitive;

/// Dispatch only implemented combinations; a mode cannot change a primitive's meaning.
pub(crate) fn check(
    cas: &mut Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    let Certificate::Boundary(mode) = &draft.certificate else {
        return Err(invalid());
    };
    match (draft.rule, mode.as_ref()) {
        (Primitive::JetPolynomial, BoundaryCertificate::Partition { .. }) => {
            crate::boundary_partition::check(cas, draft, premises, budget)?;
        }
        (Primitive::JetPolynomial, BoundaryCertificate::Polynomial { .. }) => {
            crate::jet_polynomial::check(cas, draft, premises, budget)?;
        }
        (Primitive::JetPolynomial, BoundaryCertificate::Projection) => {
            crate::jet_polynomial::projection(draft, premises)?;
        }
        (Primitive::JetPolynomial, BoundaryCertificate::Jump) => {
            crate::jet_polynomial::jump(cas, draft, premises, budget)?;
        }
        (Primitive::CaseInterior, BoundaryCertificate::Interior) => {
            crate::case_calculus::interior(cas, draft, premises, budget)?;
        }
        (Primitive::CaseInterior, BoundaryCertificate::Assemble) => {
            crate::case_calculus::assemble(cas, draft, premises, budget)?;
        }
        (Primitive::CaseBoundary, BoundaryCertificate::Point) => {
            crate::case_point::check(cas, draft, premises, budget)?;
        }
        _ => return Err(invalid()),
    }
    let mut result = ScopedConditions::default();
    for premise in premises {
        if premise.judgment.context != draft.context {
            return Err(invalid());
        }
        crate::statement::merge(cas, &mut result, &premise.conditions, budget)?;
    }
    Ok(result)
}

/// Require total local agreement, not merely agreement where both happen to be defined.
pub(crate) fn equality(
    statement: &Statement,
    function: &Handle,
    polynomial: &Handle,
    region: &Handle,
) -> Result<(), Fault> {
    let expected = Statement::Equality {
        left: function.clone(),
        right: polynomial.clone(),
        domain: region.clone(),
        domain_contract: DomainContract::DefinedAndEqual,
    };
    if *statement == expected {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Read only literal exact values at the trusted arithmetic boundary.
pub(crate) fn rational<'a>(
    cas: &'a Cas,
    value: &Handle,
) -> Result<&'a crate::number::Rational, Fault> {
    let crate::model::Operator::Rational(number) =
        &cas.state.expression(cas.checked(value)?)?.operator
    else {
        return Err(invalid());
    };
    Ok(number)
}

/// The exact ordered premise, with no implicit search through unrelated facts.
pub(crate) fn premise(premises: &[CheckedApplication], index: usize) -> Result<&Statement, Fault> {
    Ok(&premises.get(index).ok_or_else(invalid)?.judgment.statement)
}
