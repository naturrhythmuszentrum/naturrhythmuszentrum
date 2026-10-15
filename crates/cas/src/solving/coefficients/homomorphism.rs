// SPDX-License-Identifier: AGPL-3.0-only
//! Exact map hypotheses, including canonical parameter maps and selected-root evaluation.

use super::storage::{AT, domain, invalid};
use crate::budget::{Budget, BudgetKind};
use crate::error::Fault;
use crate::graph::{Cas, ExpressionAttachment, Handle};
use crate::model::Operator;
use crate::proof_model::{CheckedApplication, Statement};
use crate::solving::{
    CoefficientDomainData, CoefficientMapData, MapMode, MapTarget, SolveDraft, SolveStatement,
};

/// Parent inclusions into a monic quotient are injective without an irreducibility assertion.
fn parent_inclusion(
    cas: &Cas,
    source: &Handle,
    target: &Handle,
    budget: &mut Budget,
) -> Result<bool, Fault> {
    let mut current = target;
    loop {
        budget.work(1, AT)?;
        if current == source {
            return Ok(true);
        }
        match domain(&cas.state, current)? {
            CoefficientDomainData::Quotient { level } => {
                let SolveDraft::ExtensionLevel(extension) =
                    crate::solving::graph::object(&cas.state, level.logical_id())?
                else {
                    return Err(invalid());
                };
                current = &extension.parent;
            }
            CoefficientDomainData::Fraction { base, .. } => current = base,
            CoefficientDomainData::Gaussian => {
                return Ok(matches!(
                    domain(&cas.state, source)?,
                    CoefficientDomainData::Rational
                ));
            }
            CoefficientDomainData::Rational => return Ok(false),
        }
    }
}

/// Independent coordinate evaluation uses the exact declared keys in their fixed order.
fn parameters(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    coordinates: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if map.mode != MapMode::Specialization {
        return Err(invalid());
    }
    let CoefficientDomainData::Fraction { base, .. } = domain(&cas.state, &map.source)? else {
        return Err(invalid());
    };
    let MapTarget::ScalarAmbient { field, .. } = &map.target else {
        return Err(invalid());
    };
    if matches!(domain(&cas.state, base)?, CoefficientDomainData::Gaussian)
        && *field != crate::model::Field::Complex
    {
        return Err(invalid());
    }
    if !matches!(
        field,
        crate::model::Field::Rational | crate::model::Field::Real | crate::model::Field::Complex
    ) {
        return Err(invalid());
    }
    let SolveDraft::CoordinateSpace(product) =
        crate::solving::graph::object(&cas.state, coordinates.logical_id())?
    else {
        return Err(invalid());
    };
    if map.images.len() != product.coordinates.len() {
        return Err(invalid());
    }
    for (image, coordinate) in map.images.iter().zip(&product.coordinates) {
        budget.work(crate::graph::size(coordinate.key.len(), AT)?, AT)?;
        let expression = cas.state.expression(cas.checked(image)?)?;
        if !matches!(&expression.operator, Operator::Free(key) if key == &coordinate.key)
            || expression.header != product.header.logical_id()
        {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A selected root may evaluate a quotient only after that exact selection is known to exist.
fn existence(
    selection: &Handle,
    region: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    for premise in premises {
        budget.work(1, AT)?;
        if let Statement::Solve(statement) = &premise.judgment.statement
            && let SolveStatement::RootExistence(value) = statement.as_ref()
            && value.relation == *selection
            && value.region == *region
        {
            return Ok(());
        }
    }
    Err(invalid())
}

/// Root evaluation retains the parent coefficient interpretation from the root relation itself.
fn parent_map<'a>(
    cas: &'a Cas,
    map: &CoefficientMapData<Handle>,
    level: &Handle,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<&'a CoefficientMapData<Handle>, Fault> {
    if map.mode != MapMode::Specialization {
        return Err(invalid());
    }
    let [image] = map.images.as_slice() else {
        return Err(invalid());
    };
    let expression = cas.state.expression(cas.checked(image)?)?;
    let (Operator::RootValue { slot }, Some(ExpressionAttachment::SelectedRoot(selected))) =
        (&expression.operator, &expression.attachment)
    else {
        return Err(invalid());
    };
    let selection_handle = cas.handle(*selected);
    existence(&selection_handle, &map.region, premises, budget)?;
    let SolveDraft::RootSelection(selection) =
        crate::solving::graph::object(&cas.state, *selected)?
    else {
        return Err(invalid());
    };
    let relation_handle = selection
        .relations
        .get(usize::try_from(*slot).map_err(|_| invalid())?)
        .ok_or_else(invalid)?;
    let SolveDraft::RootRelation(relation) =
        crate::solving::graph::object(&cas.state, relation_handle.logical_id())?
    else {
        return Err(invalid());
    };
    let SolveDraft::ExtensionLevel(extension) =
        crate::solving::graph::object(&cas.state, level.logical_id())?
    else {
        return Err(invalid());
    };
    if relation.polynomial != extension.polynomial || relation.region != map.region {
        return Err(invalid());
    }
    let SolveDraft::CoefficientMap(parent) =
        crate::solving::graph::object(&cas.state, relation.map.logical_id())?
    else {
        return Err(invalid());
    };
    if (&parent.source, &parent.target) != (&extension.parent, &map.target) {
        return Err(invalid());
    }
    Ok(parent)
}

/// Validate the full map chain without assuming that every claimed embedding is injective.
pub(crate) fn check(
    cas: &Cas,
    map: &CoefficientMapData<Handle>,
    premises: &[CheckedApplication],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let mut current = map;
    let mut depth = 0_u64;
    loop {
        budget.work(1, AT)?;
        depth = depth
            .checked_add(1)
            .ok_or_else(|| Fault::capacity(BudgetKind::Depth, AT))?;
        budget.check(BudgetKind::Depth, depth, AT)?;
        for denominator in &current.denominators {
            super::proof::denominator(
                cas,
                denominator,
                &current.source,
                &current.region,
                premises,
                budget,
            )?;
        }
        if let MapTarget::CoefficientDomain { domain: target } = &current.target {
            if current.images.is_empty() && parent_inclusion(cas, &current.source, target, budget)?
            {
                return Ok(());
            }
            if current.mode == MapMode::Embedding {
                let engine = super::arithmetic::Engine {
                    state: &cas.state,
                    domain: &current.source,
                };
                let zero = engine
                    .constant(false, budget)
                    .map_err(|error| super::proof::fault(&error))?;
                super::maps::inclusion(&cas.state, &zero, current, target, budget)
                    .map_err(|error| super::proof::fault(&error))?;
                return Ok(());
            }
            return Err(invalid());
        }
        match domain(&cas.state, &current.source)? {
            CoefficientDomainData::Rational | CoefficientDomainData::Gaussian => {
                return super::proof::ground_map(cas, current, budget);
            }
            CoefficientDomainData::Fraction { coordinates, .. } => {
                return parameters(cas, current, coordinates, budget);
            }
            CoefficientDomainData::Quotient { level } => {
                current = parent_map(cas, current, level, premises, budget)?;
            }
        }
    }
}
