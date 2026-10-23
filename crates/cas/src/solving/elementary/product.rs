// SPDX-License-Identifier: AGPL-3.0-only
//! Complete finite solutions from exact factor reconstruction and strict membership.

use super::evaluate::Evaluation;
use super::proof::{AT, coordinates, invalid, premise, problem, tuple};
use super::value::Value;
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, Handle};
use crate::model::PredicateKind;
use crate::proof_model::{ApplicationDraft, CheckedApplication, DomainContract, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::storage::{reserve, vector};
use crate::solving::{
    CertificateProductCompleteness, SolutionBranchKind, SolveDraft, SolveProblemData,
    SolveStatement, StatementCoverage,
};

/// A failed optional finite proof remains undecided; exhaustion always propagates.
pub(crate) const fn decides(result: Result<(), Fault>) -> Result<bool, Fault> {
    match result {
        Ok(()) => Ok(true),
        Err(error)
            if matches!(
                error.kind,
                crate::error::FaultKind::Invalid(crate::error::InvalidCode::Rule)
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

/// One original polynomial equation is reconstructed before invoking the zero-product theorem.
pub(crate) fn equation(
    evaluation: &Evaluation<'_>,
    original: &SolveProblemData<Handle>,
    budget: &mut Budget,
) -> Result<Value, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("exclusion.equation");
    let (PredicateKind::Equal, [left, right]) =
        super::evidence::predicate(evaluation.cas, original.predicate.logical_id())?
    else {
        return Err(invalid());
    };
    evaluation.expression(*left, true, budget)?.sub(
        &evaluation.expression(*right, true, budget)?,
        &evaluation.engine,
        budget,
    )
}

/// Identity evidence must name precisely the reconstructed endpoints and the same coefficient region.
pub(crate) fn identity(
    evaluation: &Evaluation<'_>,
    slot: u32,
    left: &Value,
    right: &Value,
    budget: &mut Budget,
) -> Result<(), Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("exclusion.identity");
    let statement = &premise(evaluation.premises, slot)?.judgment.statement;
    let (first, second) = match statement {
        Statement::Equality {
            left,
            right,
            domain,
            domain_contract: DomainContract::DefinedAndEqual,
        } if domain == evaluation.region => (left, right),
        Statement::Solve(statement) => match statement.as_ref() {
            SolveStatement::CoefficientEquality(value) if value.region == *evaluation.region => {
                (&value.left, &value.right)
            }
            _ => return Err(invalid()),
        },
        _ => return Err(invalid()),
    };
    let first = evaluation.expression(first.logical_id(), true, budget)?;
    let second = evaluation.expression(second.logical_id(), true, budget)?;
    if (decides(evaluation.zero(&left.sub(&first, &evaluation.engine, budget)?, budget))?
        && decides(evaluation.zero(&right.sub(&second, &evaluation.engine, budget)?, budget))?)
        || (decides(evaluation.zero(&left.sub(&second, &evaluation.engine, budget)?, budget))?
            && decides(evaluation.zero(&right.sub(&first, &evaluation.engine, budget)?, budget))?)
    {
        return Ok(());
    }
    Err(invalid())
}

/// A finite cover contains exactly full-region finite/empty branches of this original problem.
pub(crate) fn entries(
    cas: &Cas,
    coverage: &StatementCoverage,
    budget: &mut Budget,
) -> Result<Vec<Handle>, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("inventory");
    let original = problem(cas, &coverage.problem)?;
    let SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&cas.state, cas.checked(&coverage.set)?)?
    else {
        return Err(invalid());
    };
    if set.problem != coverage.problem {
        return Err(invalid());
    }
    let mut result = vector(0, budget)?;
    for branch in &set.branches {
        budget.work(1, AT)?;
        let SolveDraft::SolutionBranch(branch) =
            crate::solving::graph::object(&cas.state, cas.checked(branch)?)?
        else {
            return Err(invalid());
        };
        if (&branch.problem, &branch.region, &branch.domain)
            != (&coverage.problem, &coverage.region, &original.domain)
        {
            return Err(invalid());
        }
        match &branch.kind {
            SolutionBranchKind::Empty => {}
            SolutionBranchKind::Finite { tuples } => {
                reserve(&mut result, tuples.len(), budget)?;
                for handle in tuples {
                    let [entry] = tuple(cas, handle)?.entries.as_slice() else {
                        return Err(invalid());
                    };
                    result.push(entry.clone());
                }
            }
            _ => return Err(invalid()),
        }
    }
    Ok(result)
}

/// Every represented tuple is an original solution, independently of the factor identity.
pub(crate) fn membership(
    evaluation: &Evaluation<'_>,
    original: &SolveProblemData<Handle>,
    entries: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("existence.membership");
    let unknowns = coordinates(evaluation.cas, &original.unknowns)?;
    let [coordinate] = unknowns.coordinates.as_slice() else {
        return Err(invalid());
    };
    for entry in entries {
        let value = evaluation.expression(entry.logical_id(), true, budget)?;
        let replacement = [(coordinate.key.as_str(), value)];
        let substituted = Evaluation {
            free: &replacement,
            ..evaluation.unbound()
        };
        super::member::original(&substituted, original, budget)?;
    }
    Ok(())
}

/// Each factor is either represented or excluded by an exact original denominator zero.
fn inclusion(
    evaluation: &Evaluation<'_>,
    original: &SolveProblemData<Handle>,
    root: &Value,
    entries: &[Handle],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for entry in entries {
        let value = evaluation.expression(entry.logical_id(), true, budget)?;
        if decides(evaluation.zero(&root.sub(&value, &evaluation.engine, budget)?, budget))? {
            return Ok(());
        }
    }
    let unknowns = coordinates(evaluation.cas, &original.unknowns)?;
    let [coordinate] = unknowns.coordinates.as_slice() else {
        return Err(invalid());
    };
    let replacement = [(
        coordinate.key.as_str(),
        root.copy(&evaluation.engine, budget)?,
    )];
    let substituted = Evaluation {
        free: &replacement,
        ..evaluation.unbound()
    };
    for denominator in &original.denominators {
        let value = substituted.expression(denominator.logical_id(), true, budget)?;
        if decides(evaluation.zero(&value, budget))? {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Reconstruct every positive factor power; repeated tuple syntax never supplies multiplicity.
pub(crate) fn factors(
    evaluation: &Evaluation<'_>,
    original: &SolveProblemData<Handle>,
    certificate: &CertificateProductCompleteness,
    entries: &[Handle],
    budget: &mut Budget,
) -> Result<Value, Fault> {
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("exclusion.factors");
    let input = super::transport::unknowns(evaluation, original, budget)?;
    let [unknown] = input.as_slice() else {
        return Err(invalid());
    };
    let mut result = evaluation.expression(certificate.scalar.logical_id(), true, budget)?;
    evaluation.nonzero(&result, budget)?;
    for factor in &certificate.factors {
        if factor.multiplicity == 0 {
            return Err(invalid());
        }
        budget.check(
            BudgetKind::PolynomialDegree,
            u64::from(factor.multiplicity),
            AT,
        )?;
        let root = evaluation.expression(factor.root.logical_id(), true, budget)?;
        inclusion(evaluation, original, &root, entries, budget)?;
        result = result.mul(
            &unknown.sub(&root, &evaluation.engine, budget)?.pow(
                factor.multiplicity,
                &evaluation.engine,
                budget,
            )?,
            &evaluation.engine,
            budget,
        )?;
    }
    Ok(result)
}

/// Nonzero scalar, exact identity and both inclusions justify complete coverage in a scalar field.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateProductCompleteness,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Coverage(coverage) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&coverage.problem, &coverage.set, &coverage.region)
        != (&certificate.problem, &certificate.set, &certificate.region)
    {
        return Err(invalid());
    }
    let original = problem(cas, &certificate.problem)?;
    super::proof::region(cas, &draft.context, original, &certificate.region)?;
    super::member::contained(cas, &certificate.region, &original.region, budget)?;
    if !matches!(
        original.ambient,
        crate::model::Field::Rational | crate::model::Field::Real | crate::model::Field::Complex
    ) {
        return Err(invalid());
    }
    for slot in &certificate.existence {
        let Statement::Solve(statement) = &premise(premises, *slot)?.judgment.statement else {
            return Err(invalid());
        };
        if !matches!(statement.as_ref(), SolveStatement::RootExistence(value) if value.region == certificate.region)
        {
            return Err(invalid());
        }
    }
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &certificate.region,
        premises,
        free: &[],
        bound: &[],
    };
    let entries = entries(cas, coverage, budget)?;
    membership(&evaluation, original, &entries, budget)?;
    let product = factors(&evaluation, original, certificate, &entries, budget)?;
    let original_polynomial = equation(&evaluation, original, budget)?;
    identity(
        &evaluation,
        certificate.product,
        &original_polynomial,
        &product,
        budget,
    )
}
