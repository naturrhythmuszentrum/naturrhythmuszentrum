// SPDX-License-Identifier: AGPL-3.0-only
//! Total zero/sign partitions and exhaustive regional coverage elimination.

use super::proof::{AT, invalid, premise};
use crate::budget::Budget;
use crate::domain_model::RegionExpr;
use crate::error::Fault;
use crate::graph::{Cas, Handle, LogicalId};
use crate::model::{AtomicSort, Field, Operator, PredicateKind, Sort};
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::{
    CertificateTotalPredicateSplit, PredicateSplitMode, SolveDraft, SolveStatement,
};

/// A zero guard must use the split expression and an exact zero literal.
fn zero(
    cas: &Cas,
    predicate: LogicalId,
    expression: LogicalId,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let (kind, arguments) = super::evidence::predicate(cas, predicate)?;
    let (PredicateKind::Equal, [left, right]) = (kind, arguments) else {
        return Ok(false);
    };
    let constant = if *left == expression {
        *right
    } else if *right == expression {
        *left
    } else {
        return Ok(false);
    };
    budget.work(1, AT)?;
    Ok(
        matches!(&cas.state.expression(constant)?.operator, Operator::Rational(value) if value.is_zero()),
    )
}

/// Ordered children reconstruct precisely zero/nonzero or negative/zero/positive.
fn child(
    cas: &Cas,
    certificate: &CertificateTotalPredicateSplit,
    handle: &Handle,
    position: usize,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let region = cas.state.region(cas.checked(handle)?)?;
    let source = cas.state.region(cas.checked(&certificate.region)?)?;
    if (&region.ambient, &region.scope, region.header)
        != (&source.ambient, &source.scope, source.header)
    {
        return Err(invalid());
    }
    let (base, predicate, truth) = match &region.expression {
        RegionExpr::GuardTrue { base, predicate } => (*base, *predicate, true),
        RegionExpr::GuardFalse { base, predicate } => (*base, *predicate, false),
        _ => return Err(invalid()),
    };
    if base != certificate.region.logical_id() {
        return Err(invalid());
    }
    let expression = certificate.expression.logical_id();
    if certificate.mode == PredicateSplitMode::ZeroNonzero {
        if zero(cas, predicate, expression, budget)? && truth == (position == 0) {
            return Ok(());
        }
        let (kind, arguments) = super::evidence::predicate(cas, predicate)?;
        if position == 1 && truth && *kind == PredicateKind::NonZero && arguments == [expression] {
            return Ok(());
        }
    } else if position == 1 {
        if truth && zero(cas, predicate, expression, budget)? {
            return Ok(());
        }
    } else if truth {
        let (kind, arguments) = super::evidence::predicate(cas, predicate)?;
        if let (PredicateKind::Positive, [argument]) = (kind, arguments) {
            if position == 2 && *argument == expression {
                return Ok(());
            }
            if position == 0 {
                let term = cas.state.expression(*argument)?;
                if term.operator == Operator::Neg && term.operands == [expression] {
                    return Ok(());
                }
            }
        }
    }
    Err(invalid())
}

/// Set equality is bounded and does not identify merely provably equal regions.
fn same_members(left: &[Handle], right: &[Handle], budget: &mut Budget) -> Result<bool, Fault> {
    budget.work(
        crate::graph::size(left.len(), AT)?
            .checked_mul(crate::graph::size(right.len(), AT)?)
            .ok_or_else(invalid)?,
        AT,
    )?;
    Ok(left.len() == right.len()
        && left.iter().all(|member| right.contains(member))
        && right.iter().all(|member| left.contains(member)))
}

/// A missing degree-zero or coefficient-zero branch cannot disappear during cover elimination.
fn eliminate(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateTotalPredicateSplit,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Coverage(coverage) = statement.as_ref() else {
        return Err(invalid());
    };
    let original = super::proof::problem(cas, &coverage.problem)?;
    super::proof::region(cas, &draft.context, original, &certificate.region)?;
    super::member::contained(cas, &certificate.region, &original.region, budget)?;
    let Statement::Cover { domain, members } =
        &premise(premises, certificate.cover.ok_or_else(invalid)?)?
            .judgment
            .statement
    else {
        return Err(invalid());
    };
    if domain != &certificate.region
        || coverage.region != certificate.region
        || !same_members(members, &certificate.children, budget)?
        || certificate.children.len() != certificate.branches.len()
    {
        return Err(invalid());
    }
    let SolveDraft::SolutionSet(target) =
        crate::solving::graph::object(&cas.state, cas.checked(&coverage.set)?)?
    else {
        return Err(invalid());
    };
    if target.problem != coverage.problem {
        return Err(invalid());
    }
    let mut branches =
        crate::solving::coefficients::storage::vector(target.branches.len(), budget)?;
    for (region, slot) in certificate.children.iter().zip(&certificate.branches) {
        let Statement::Solve(statement) = &premise(premises, *slot)?.judgment.statement else {
            return Err(invalid());
        };
        let SolveStatement::Coverage(part) = statement.as_ref() else {
            return Err(invalid());
        };
        if part.problem != coverage.problem {
            return Err(invalid());
        }
        closed_region(cas, &part.region, region, budget)?;
        let SolveDraft::SolutionSet(set) =
            crate::solving::graph::object(&cas.state, cas.checked(&part.set)?)?
        else {
            return Err(invalid());
        };
        crate::solving::coefficients::storage::reserve(&mut branches, set.branches.len(), budget)?;
        for member in &set.branches {
            if !branches.contains(member) {
                branches.push(member.clone());
            }
        }
    }
    if !same_members(&branches, &target.branches, budget)? {
        return Err(invalid());
    }
    Ok(())
}

/// Definedness is checked before invoking the total classical alternatives.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateTotalPredicateSplit,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.mode == PredicateSplitMode::EliminateCover {
        return eliminate(cas, draft, certificate, premises, budget);
    }
    if certificate.cover.is_some() || !certificate.branches.is_empty() {
        return Err(invalid());
    }
    let Statement::Cover { domain, members } = &draft.conclusion else {
        return Err(invalid());
    };
    let count = if certificate.mode == PredicateSplitMode::ZeroNonzero {
        2
    } else {
        3
    };
    if domain != &certificate.region
        || certificate.children.len() != count
        || !same_members(members, &certificate.children, budget)?
    {
        return Err(invalid());
    }
    let expression = cas
        .state
        .expression(cas.checked(&certificate.expression)?)?;
    let region = cas.state.region(cas.checked(&certificate.region)?)?;
    let context = cas.state.context(cas.checked(&draft.context)?)?;
    if (expression.header, &expression.scope, context.admissible)
        != (
            region.header,
            &region.scope,
            certificate.region.logical_id(),
        )
    {
        return Err(invalid());
    }
    if !matches!(
        expression.sort,
        Sort::Value(AtomicSort::Scalar(
            Field::Rational | Field::Real | Field::Complex
        ))
    ) {
        return Err(invalid());
    }
    if certificate.mode == PredicateSplitMode::RealSigns
        && !matches!(
            expression.sort,
            Sort::Value(AtomicSort::Scalar(Field::Rational | Field::Real))
        )
    {
        return Err(invalid());
    }
    super::total::check(
        cas,
        certificate.expression.logical_id(),
        &certificate.region,
        premises,
        budget,
    )?;
    for (position, region) in certificate.children.iter().enumerate() {
        child(cas, certificate, region, position, budget)?;
    }
    Ok(())
}

/// Closing a local context retains its exact meet; the meet must equal the covered child region.
fn closed_region(
    cas: &Cas,
    proved: &Handle,
    child: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if proved == child {
        return Ok(());
    }
    let RegionExpr::Meet(members) = &cas.state.region(proved.logical_id())?.expression else {
        return Err(invalid());
    };
    if !members.contains(&child.logical_id()) {
        return Err(invalid());
    }
    for member in members {
        super::member::contained(cas, child, &cas.handle(*member), budget)?;
    }
    Ok(())
}
