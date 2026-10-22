// SPDX-License-Identifier: AGPL-3.0-only
//! Structural exact presentations retain all root and family provenance.

mod compose;

use super::evaluate::Evaluation;
use super::proof::{AT, invalid, problem, tuple};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle, LogicalId};
use crate::model::Operator;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::storage::{polynomial, reserve, vector};
use crate::solving::{
    CertificateRootSelectionRefinement, RefinementTargetKind, SolutionBranchKind, SolveDraft,
    SolveForm, SolveStatement,
};

/// A root of unity needs its exact order theorem; other radicals need a binomial defining relation.
fn root(
    evaluation: &Evaluation<'_>,
    id: LogicalId,
    selected: LogicalId,
    slot: u32,
    form: SolveForm,
    pending: &mut Vec<LogicalId>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    evaluation.existence(selected, budget)?;
    let selected = super::selected::selection(evaluation.cas, &evaluation.cas.handle(selected))?;
    let relation = super::selected::relation(
        evaluation.cas,
        selected
            .relations
            .get(usize::try_from(slot).map_err(|_| invalid())?)
            .ok_or_else(invalid)?,
    )?;
    if form == SolveForm::Radicals && selected.cell.is_some() {
        return Err(invalid());
    }
    let polynomial = polynomial(&evaluation.cas.state, &relation.polynomial)?;
    let order_proved = evaluation.premises.iter().any(|premise| matches!(&premise.judgment.statement, Statement::Solve(statement) if matches!(statement.as_ref(), SolveStatement::RootOrder(value) if value.value.logical_id() == id && &value.region == evaluation.region)));
    if form == SolveForm::Radicals && !order_proved {
        let degree = &polynomial.terms.first().ok_or_else(invalid)?.exponents;
        if degree.first().is_none_or(|power| *power == 0)
            || polynomial
                .terms
                .iter()
                .any(|term| &term.exponents != degree && term.exponents != [0])
        {
            return Err(invalid());
        }
    }
    let SolveDraft::CoefficientMap(map) =
        crate::solving::graph::object(&evaluation.cas.state, relation.map.logical_id())?
    else {
        return Err(invalid());
    };
    if form == SolveForm::Radicals && !order_proved {
        let leading = &polynomial.terms.first().ok_or_else(invalid)?.coefficient;
        evaluation.nonzero(&evaluation.mapped(leading, map, budget)?, budget)?;
    }
    reserve(pending, map.images.len(), budget)?;
    pending.extend(map.images.iter().map(Handle::logical_id));
    crate::solving::coefficients::proof::defined(
        evaluation.cas,
        &relation.polynomial,
        evaluation.region,
        evaluation.premises,
        budget,
    )
}

/// The explicit stack visits nested root maps as well as visible arithmetic expressions.
fn expression(
    evaluation: &Evaluation<'_>,
    handle: &Handle,
    form: SolveForm,
    family: bool,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let region = evaluation
        .cas
        .state
        .region(evaluation.region.logical_id())?;
    let crate::domain_model::AmbientDomain::Product { coordinates } = &region.ambient else {
        return Err(invalid());
    };
    let allowed = super::proof::coordinates(evaluation.cas, coordinates)?;
    let mut pending = vector(1, budget)?;
    pending.push(handle.logical_id());
    while let Some(id) = pending.pop() {
        budget.work(1, AT)?;
        let value = evaluation.cas.state.expression(id)?;
        match (&value.operator, &value.attachment) {
            (Operator::Free(key), None)
                if allowed.coordinates.iter().any(|entry| &entry.key == key) => {}
            (Operator::Bound { depth: 0, .. }, None) if family => {}
            (
                Operator::Rational(_)
                | Operator::Add
                | Operator::Sub
                | Operator::Mul
                | Operator::Div
                | Operator::Neg,
                None,
            ) => {}
            (Operator::IntegerPower(power), None) if *power > 0 => {}
            (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) => {
                root(evaluation, id, *selected, *slot, form, &mut pending, budget)?;
            }
            (
                Operator::ExactCoefficient,
                Some(ExpressionAttachment::ExactCoefficient { value, map }),
            ) => {
                let SolveDraft::CoefficientMap(map) =
                    crate::solving::graph::object(&evaluation.cas.state, *map)?
                else {
                    return Err(invalid());
                };
                crate::solving::coefficients::proof::identity_map(
                    evaluation.cas,
                    map,
                    evaluation.premises,
                    budget,
                )?;
                crate::solving::coefficients::proof::defined(
                    evaluation.cas,
                    &evaluation.cas.handle(*value),
                    evaluation.region,
                    evaluation.premises,
                    budget,
                )?;
                reserve(&mut pending, map.images.len(), budget)?;
                pending.extend(map.images.iter().map(Handle::logical_id));
            }
            _ => return Err(invalid()),
        }
        reserve(&mut pending, value.operands.len(), budget)?;
        pending.extend(&value.operands);
    }
    Ok(())
}

/// Family bodies remain under their declared binder and require universal membership evidence.
fn family(
    evaluation: &Evaluation<'_>,
    problem: &Handle,
    handle: &Handle,
    form: SolveForm,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let proved = evaluation.premises.iter().any(|premise| matches!(&premise.judgment.statement, Statement::Solve(statement) if matches!(statement.as_ref(), SolveStatement::FamilyMembership(value) if (&value.problem, &value.family, &value.region) == (problem, handle, evaluation.region))));
    if !proved {
        return Err(invalid());
    }
    let map = super::proof::family(evaluation.cas, handle)?;
    for entry in &tuple(evaluation.cas, &map.body)?.entries {
        expression(evaluation, entry, form, true, budget)?;
    }
    Ok(())
}

/// Every branch is inspected; an unresolved general algebraic placeholder cannot masquerade as a radical.
fn branches(
    evaluation: &Evaluation<'_>,
    set: &crate::solving::SolutionSetData<Handle>,
    form: SolveForm,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for handle in &set.branches {
        budget.work(1, AT)?;
        let SolveDraft::SolutionBranch(branch) =
            crate::solving::graph::object(&evaluation.cas.state, handle.logical_id())?
        else {
            return Err(invalid());
        };
        if branch.region != *evaluation.region {
            return Err(invalid());
        }
        match &branch.kind {
            SolutionBranchKind::Finite { tuples } => {
                if form == SolveForm::ExactFamily {
                    return Err(invalid());
                }
                for handle in tuples {
                    for entry in &tuple(evaluation.cas, handle)?.entries {
                        evaluation.expression(entry.logical_id(), true, budget)?;
                        expression(evaluation, entry, form, false, budget)?;
                    }
                }
            }
            SolutionBranchKind::Image { family: handle } => {
                family(evaluation, &set.problem, handle, form, budget)?;
            }
            SolutionBranchKind::Empty => {}
            SolutionBranchKind::Algebraic { selection }
                if matches!(form, SolveForm::AnyExact | SolveForm::AlgebraicRoots) =>
            {
                evaluation.existence(selection.logical_id(), budget)?;
            }
            _ => return Err(invalid()),
        }
    }
    Ok(())
}

/// Presentation is an exact representation theorem, separate from membership and completeness.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRootSelectionRefinement,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.target_kind != RefinementTargetKind::SolutionSet
        || !certificate.witnesses.is_empty()
    {
        return Err(invalid());
    }
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let SolveStatement::Presentation(claim) = statement.as_ref() else {
        return Err(invalid());
    };
    if (&claim.set, Some(claim.form)) != (&certificate.target, certificate.form)
        || claim.form == SolveForm::LocalBranch
    {
        return Err(invalid());
    }
    let SolveDraft::SolutionSet(set) =
        crate::solving::graph::object(&cas.state, claim.set.logical_id())?
    else {
        return Err(invalid());
    };
    let original = problem(cas, &set.problem)?;
    if claim.form == SolveForm::RealExact {
        if original.ambient != crate::model::Field::Real {
            return Err(invalid());
        }
        crate::solving::real::closure::evidence(cas, &claim.set, premises, budget)?;
    }
    super::proof::region(cas, &draft.context, original, &claim.region)?;
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &original.coefficient_domain,
        },
        region: &claim.region,
        premises,
        free: &[],
        bound: &[],
    };
    if certificate.compatibility.is_empty() {
        branches(&evaluation, set, claim.form, budget)
    } else {
        compose::check(&evaluation, draft, certificate, set, budget)
    }
}
