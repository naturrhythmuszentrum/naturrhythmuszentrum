// SPDX-License-Identifier: AGPL-3.0-only
//! Exact finite real restriction with explicit cross-header ground-polynomial reconstruction.

use super::evaluate::Evaluation;
use super::proof::{AT, coordinates, invalid, premise, problem};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::Field;
use crate::proof_model::{ApplicationDraft, CheckedApplication, ScopedConditions, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::forms::Variable;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::{
    CertificateRealRestriction, SolutionBranchKind, SolveDraft, SolveStatement, StatementCoverage,
};

/// Foreign-context facts are usable only when unconditional; no hypothesis is borrowed by display name.
pub(crate) fn conditions(
    cas: &Cas,
    draft: &ApplicationDraft,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<ScopedConditions, Fault> {
    if !draft.substitutions.is_empty() {
        return Err(invalid());
    }
    budget.solving(BudgetKind::SolveIdentities, 1, AT)?;
    let mut result = ScopedConditions::default();
    for previous in premises {
        if previous.judgment.context == draft.context {
            crate::statement::merge(cas, &mut result, &previous.conditions, budget)?;
        } else if !previous.conditions.hypotheses.is_empty()
            || !previous.conditions.residuals.is_empty()
        {
            return Err(invalid());
        }
    }
    Ok(result)
}

/// A whole coefficient product may be written as an explicit singleton union for its cover theorem.
fn whole(cas: &Cas, handle: &Handle) -> Result<(), Fault> {
    match &cas.state.region(handle.logical_id())?.expression {
        RegionExpr::Ambient => Ok(()),
        RegionExpr::Union(members) if members.len() == 1 => {
            if matches!(
                cas.state
                    .region(*members.first().ok_or_else(invalid)?)?
                    .expression,
                RegionExpr::Ambient
            ) {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        _ => Err(invalid()),
    }
}

/// Explicit ordered unknown substitution transports only ground coefficients across the two headers.
fn ground_equation(
    evaluation: &Evaluation<'_>,
    original: &crate::solving::SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    let unknowns = coordinates(evaluation.cas, &original.unknowns)?;
    let [coordinate] = unknowns.coordinates.as_slice() else {
        return Err(invalid());
    };
    let input = [(
        coordinate.key.as_str(),
        Value::variable(Variable::Slot(u32::MAX), &evaluation.engine, budget)?,
    )];
    let lifted = Evaluation {
        free: &input,
        ..evaluation.unbound()
    };
    let equation = super::product::equation(&lifted, original, budget)?;
    for term in equation
        .numerator
        .terms
        .iter()
        .chain(&equation.denominator.terms)
    {
        if term
            .powers
            .iter()
            .any(|(variable, _)| *variable != Variable::Slot(u32::MAX))
        {
            return Err(invalid());
        }
    }
    if !original.denominators.is_empty()
        || !matches!(
            evaluation
                .cas
                .state
                .region(original.domain.logical_id())?
                .expression,
            RegionExpr::Ambient
        )
    {
        return Err(invalid());
    }
    whole(evaluation.cas, &original.region)?;
    Ok(equation)
}

/// Each listed source branch is classified in full, never by one sampled member.
fn classify(
    evaluation: &Evaluation<'_>,
    branch: &Handle,
    retained: bool,
    output: &mut Vec<Value>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let SolveDraft::SolutionBranch(branch) =
        crate::solving::graph::object(&evaluation.cas.state, branch.logical_id())?
    else {
        return Err(invalid());
    };
    if branch.region != *evaluation.region {
        return Err(invalid());
    }
    let tuples = match &branch.kind {
        SolutionBranchKind::Finite { tuples } => tuples,
        SolutionBranchKind::Empty => return Ok(()),
        _ => return Err(invalid()),
    };
    for handle in tuples {
        let [entry] = super::proof::tuple(evaluation.cas, handle)?
            .entries
            .as_slice()
        else {
            return Err(invalid());
        };
        if retained {
            let parts = super::complex::expression(evaluation, entry.logical_id(), budget)?;
            evaluation.zero(&parts.imaginary, budget)?;
            reserve(output, 1, budget)?;
            output.push(parts.real);
        } else {
            let direct = super::complex::expression(evaluation, entry.logical_id(), budget)
                .and_then(|parts| evaluation.nonzero(&parts.imaginary, budget));
            if !super::product::decides(direct)? {
                super::nonreal::check(evaluation, entry, budget)?;
            }
        }
    }
    Ok(())
}

/// Exact set equality follows the complete retained/rejected partition and both inclusion checks.
fn partition(
    evaluation: &Evaluation<'_>,
    coverage: &StatementCoverage,
    certificate: &CertificateRealRestriction,
    budget: &mut Budget,
) -> Result<Vec<Value>, Fault> {
    let SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&evaluation.cas.state, coverage.set.logical_id())?
    else {
        return Err(invalid());
    };
    if certificate
        .retained
        .len()
        .checked_add(certificate.rejected.len())
        != Some(set.branches.len())
    {
        return Err(invalid());
    }
    for branch in &set.branches {
        let count = certificate
            .retained
            .iter()
            .chain(&certificate.rejected)
            .filter(|candidate| *candidate == branch)
            .count();
        if count != 1 {
            return Err(invalid());
        }
    }
    let mut values = vector(0, budget)?;
    for branch in &certificate.retained {
        classify(evaluation, branch, true, &mut values, budget)?;
    }
    for branch in &certificate.rejected {
        classify(evaluation, branch, false, &mut values, budget)?;
    }
    Ok(values)
}

/// The source coefficient cover is itself an earlier theorem over the exact original region.
fn cover(
    cas: &Cas,
    coverage: &StatementCoverage,
    certificate: &CertificateRealRestriction,
    premises: &[CheckedApplication],
) -> Result<(), Fault> {
    let Statement::Cover { domain, members } =
        &premise(premises, certificate.cover)?.judgment.statement
    else {
        return Err(invalid());
    };
    if domain != &coverage.region {
        return Err(invalid());
    }
    whole(cas, domain)?;
    if members.is_empty() {
        return Err(invalid());
    }
    for slot in &certificate.classification {
        premise(premises, *slot)?;
    }
    Ok(())
}

/// Complete real coverage is the exact real subset of an unconditional complete complex ground problem.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRealRestriction,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Coverage(target) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&target.problem, &target.set) != (&certificate.problem, &certificate.set) {
        return Err(invalid());
    }
    let Statement::Solve(statement) = &premise(premises, certificate.coverage)?.judgment.statement
    else {
        return Err(invalid());
    };
    let SolveStatement::Coverage(source) = statement.as_ref() else {
        return Err(invalid());
    };
    let real = problem(cas, &target.problem)?;
    let complex = problem(cas, &source.problem)?;
    if (&real.ambient, &complex.ambient, &real.coefficient_domain)
        != (&Field::Real, &Field::Complex, &complex.coefficient_domain)
    {
        return Err(invalid());
    }
    super::proof::region(cas, &draft.context, real, &target.region)?;
    whole(cas, &target.region)?;
    cover(cas, source, certificate, premises)?;
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &real.coefficient_domain,
        },
        region: &target.region,
        premises,
        free: &[],
        bound: &[],
    };
    let lifted = Evaluation {
        region: &source.region,
        ..evaluation.unbound()
    };
    if !ground_equation(&evaluation, real, budget)?.equals(
        &ground_equation(&lifted, complex, budget)?,
        &evaluation.engine,
        budget,
    )? {
        return Err(invalid());
    }
    let expected = partition(&lifted, source, certificate, budget)?;
    let entries = super::product::entries(cas, target, budget)?;
    super::product::membership(&evaluation, real, &entries, budget)?;
    let mut actual = vector(entries.len(), budget)?;
    for entry in &entries {
        actual.push(evaluation.expression(entry.logical_id(), true, budget)?);
    }
    super::transport::same_values(&evaluation, &expected, &actual, budget)?;
    super::transport::same_values(&evaluation, &actual, &expected, budget)
}
