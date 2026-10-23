// SPDX-License-Identifier: AGPL-3.0-only
//! Constructive joint root choices, with a separate universal uniqueness check.

use super::evaluate::Evaluation;
use super::proof::{invalid, premise};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::Cas;
use crate::proof_model::{ApplicationDraft, CheckedApplication, Statement};
use crate::solving::coefficients::arithmetic::Engine;
use crate::solving::coefficients::storage::{polynomial, vector};
use crate::solving::{
    CertificateRootSelectionRefinement, RefinementMode, RefinementTargetKind, SelectionMode,
    SolveDraft, SolveStatement,
};

/// Every witness satisfies its own relation and the complete joint predicate under one binder.
fn witnesses(
    evaluation: &Evaluation<'_>,
    certificate: &CertificateRootSelectionRefinement,
    selection: &crate::solving::RootSelectionData<crate::graph::Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if selection.relations.len() != certificate.witnesses.len() {
        return Err(invalid());
    }
    let mut values = vector(certificate.witnesses.len(), budget)?;
    for (witness, handle) in certificate.witnesses.iter().zip(&selection.relations) {
        let value = evaluation.expression(witness.logical_id(), true, budget)?;
        let relation = super::selected::relation(evaluation.cas, handle)?;
        if evaluation.cas.state.expression(witness.logical_id())?.sort
            != crate::model::Sort::Value(crate::model::AtomicSort::Scalar(relation.ambient.clone()))
        {
            return Err(invalid());
        }
        evaluation.zero(&evaluation.root_equation(&value, relation, budget)?, budget)?;
        values.push(value);
    }
    let joint = Evaluation {
        bound: &values,
        ..evaluation.unbound()
    };
    joint.predicate(selection.predicate.logical_id(), budget)
}

/// Linear defining relations with specialized nonzero leading coefficients have at most one joint tuple.
fn unique(
    evaluation: &Evaluation<'_>,
    selection: &crate::solving::RootSelectionData<crate::graph::Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    for handle in &selection.relations {
        let relation = super::selected::relation(evaluation.cas, handle)?;
        let stored = polynomial(&evaluation.cas.state, &relation.polynomial)?;
        let leading = stored.terms.first().ok_or_else(invalid)?;
        if leading.exponents != [1] {
            return Err(invalid());
        }
        let SolveDraft::CoefficientMap(map) =
            crate::solving::graph::object(&evaluation.cas.state, relation.map.logical_id())?
        else {
            return Err(invalid());
        };
        evaluation.nonzero(
            &evaluation.mapped(&leading.coefficient, map, budget)?,
            budget,
        )?;
    }
    Ok(())
}

/// A principal label or isolated interval cannot replace a constructive compatibility proof.
pub(crate) fn check(
    cas: &Cas,
    draft: &ApplicationDraft,
    certificate: &CertificateRootSelectionRefinement,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    if certificate.mode == RefinementMode::Presentation {
        return super::presentation::check(cas, draft, certificate, premises, budget);
    }
    if certificate.target_kind != RefinementTargetKind::Selection || certificate.form.is_some() {
        return Err(invalid());
    }
    #[cfg(test)]
    let _attribution = crate::testing::work_trace::Scope::phase("existence");
    let selected = super::selected::selection(cas, &certificate.target)?;
    if !matches!(
        selected.mode,
        SelectionMode::Any | SelectionMode::Constrained
    ) {
        return Err(invalid());
    }
    budget.solving(
        BudgetKind::SelectionSlots,
        crate::graph::size(selected.relations.len(), super::proof::AT)?,
        super::proof::AT,
    )?;
    let first = super::selected::relation(cas, selected.relations.first().ok_or_else(invalid)?)?;
    let Statement::Solve(statement) = &draft.conclusion else {
        return Err(invalid());
    };
    let valid = match (certificate.mode, statement.as_ref()) {
        (RefinementMode::Existence, SolveStatement::RootExistence(value)) => {
            (&value.relation, &value.region) == (&certificate.target, &first.region)
        }
        (RefinementMode::Unique, SolveStatement::SelectionUnique(value)) => {
            (&value.selection, &value.region) == (&certificate.target, &first.region)
        }
        _ => false,
    };
    if !valid
        || cas.state.context(cas.checked(&draft.context)?)?.admissible != first.region.logical_id()
    {
        return Err(invalid());
    }
    for slot in &certificate.compatibility {
        premise(premises, *slot)?;
    }
    let stored = polynomial(&cas.state, &first.polynomial)?;
    let evaluation = Evaluation {
        cas,
        engine: Engine {
            state: &cas.state,
            domain: &stored.domain,
        },
        region: &first.region,
        premises,
        free: &[],
        bound: &[],
    };
    if first.ambient == crate::model::Field::Complex {
        let mut components = super::complex::Components::new(&evaluation, budget)?;
        components.premises(&certificate.compatibility, budget)?;
        components.witnesses(&certificate.witnesses, selected, budget)?;
    } else {
        witnesses(&evaluation, certificate, selected, budget)?;
    }
    if certificate.mode == RefinementMode::Unique {
        #[cfg(test)]
        let _uniqueness = crate::testing::work_trace::Scope::phase("exclusion.unique");
        unique(&evaluation, selected, budget)?;
    }
    Ok(())
}
