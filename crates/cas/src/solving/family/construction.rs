// SPDX-License-Identifier: AGPL-3.0-only
//! Producer proposals keep Real declarations in their own space.

use super::{body, invalid};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::radicals::construction::Construction;
use crate::solving::{FamilyArgument, FamilyArgumentRole, FamilyExpressionData};

/// Declared leaves keep their own field; mixed arithmetic becomes a native local case.
pub(crate) fn expression(
    cas: &mut Cas,
    construction: &Construction,
    operator: &Operator,
    operands: &[Handle],
    bound: bool,
    budget: &mut Budget,
) -> Result<Option<Handle>, Fault> {
    if bound {
        return Ok(None);
    }
    if let Operator::Free(key) = operator {
        let header = cas.state.header(construction.header.logical_id())?;
        let declaration = header
            .environment
            .declarations
            .iter()
            .find(|value| value.key == *key)
            .ok_or_else(invalid)?;
        if declaration.sort == Sort::Value(AtomicSort::Scalar(Field::Real)) {
            return super::build::expression(
                cas,
                &construction.header,
                &Vec::new(),
                operator.clone(),
                Field::Real,
                &[],
                budget,
            )
            .map(Some);
        }
    }
    let mixed = operands.iter().any(|value| {
        cas.state
            .expression(value.logical_id())
            .is_ok_and(|value| value.sort == Sort::Value(AtomicSort::Scalar(Field::Real)))
    });
    if !mixed {
        return Ok(None);
    }
    let over = cas
        .state
        .region(construction.region.logical_id())?
        .lies_over
        .clone()
        .ok_or_else(invalid)?;
    let family = cas.handle(over.family);
    let ambient = super::mapping::family_data(&cas.state, &family)?
        .fibre_space
        .clone();
    let mut arguments = crate::solving::coefficients::storage::vector(operands.len(), budget)?;
    for operand in operands {
        let (value, role) = argument(cas, operand)?;
        arguments.push(FamilyArgument {
            role,
            value: Reference::Existing(value),
        });
    }
    cas.family_expression(
        &FamilyExpressionData {
            family: Reference::Existing(family),
            ambient: Reference::Existing(ambient),
            region: Reference::Existing(construction.region.clone()),
            scope: Vec::new(),
            operator: operator.clone(),
            result: Sort::Value(AtomicSort::Scalar(Field::Complex)),
            arguments,
        },
        budget,
    )
    .map(Some)
}

/// Nested native views retain the underlying family's original argument association.
fn argument(cas: &Cas, value: &Handle) -> Result<(Handle, FamilyArgumentRole), Fault> {
    if body(cas, value.logical_id())?.is_some() {
        return Ok((value.clone(), FamilyArgumentRole::Family));
    }
    let expression = cas.state.expression(value.logical_id())?;
    if let [arm] = expression.arms.as_slice()
        && body(cas, cas.state.arm(*arm)?.value)?.is_some()
    {
        return Ok((
            cas.handle(cas.state.arm(*arm)?.value),
            FamilyArgumentRole::Family,
        ));
    }
    let role = if expression.sort == Sort::Value(AtomicSort::Scalar(Field::Real)) {
        FamilyArgumentRole::Base
    } else {
        FamilyArgumentRole::Fibre
    };
    Ok((value.clone(), role))
}

/// Each local solver frame expands its source and current projection obligations explicitly.
pub(crate) fn projection(
    cas: &mut Cas,
    construction: &mut Construction,
    problem: &Handle,
    budget: &mut Budget,
) -> Result<(), Fault> {
    use crate::proof_model::{Certificate, Statement};
    let predicate = crate::solving::elementary::proof::problem(cas, problem)?
        .predicate
        .clone();
    let mut domains = super::recipes::regions(
        cas,
        &Statement::PredicateFact {
            predicate,
            region: construction.region.clone(),
        },
        budget,
    )?;
    if !domains.contains(&construction.region) {
        domains.push(construction.region.clone());
    }
    for domain in domains {
        let lift = super::recipes::pullback(cas, &domain, budget)?;
        crate::solving::coefficients::storage::reserve(&mut construction.steps, 1, budget)?;
        construction
            .steps
            .push(crate::proof_model::ApplicationDraft {
                rule: crate::rule::Primitive::RegionConstruct,
                context: construction.context.clone(),
                conclusion: Statement::Subset {
                    subset: domain,
                    superset: lift,
                },
                premises: Vec::new(),
                substitutions: Vec::new(),
                certificate: Certificate::None,
            });
    }
    Ok(())
}
