// SPDX-License-Identifier: AGPL-3.0-only
//! Exact graph edges, scoped metadata and non-authoritative statement syntax.

use super::admission::{base, cell, family, invalid, object};
use super::{EvaluationTarget, FamilyArgumentRole, RealEndpoint};
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Edge, Handle, LogicalId, ScalarEdge, State, VertexKind};
use crate::model::{AtomicSort, Field, Scope, Sort};
use crate::solving::{SolveDraft, SolveStatement};

/// Retain one typed semantic edge after reserving its storage.
fn push(result: &mut Vec<(LogicalId, Edge)>, value: &Handle, role: Edge) -> Result<(), Fault> {
    result
        .try_reserve(1)
        .map_err(|_| Fault::allocation(super::admission::AT))?;
    result.push((value.logical_id(), role));
    Ok(())
}

/// Every semantic reference has an exact typed graph edge.
pub(crate) fn edges(
    value: &SolveDraft<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    match value {
        SolveDraft::ScalarSpace(_) => {}
        SolveDraft::SpacePlacement(value) => edges_space_placement(value, result)?,
        SolveDraft::ScalarPiece(value) => edges_scalar_piece(value, result)?,
        SolveDraft::ParameterBase(value) => edges_parameter_base(value, result)?,
        SolveDraft::SpaceFamily(value) => edges_space_family(value, result)?,
        SolveDraft::FibrePiece(value) => edges_fibre_piece(value, result)?,
        SolveDraft::RealCell(value) => edges_real_cell(value, result)?,
        SolveDraft::FamilyExpression(value) => edges_family_expression(value, result)?,
        SolveDraft::CoefficientEvaluation(value) => edges_coefficient_evaluation(value, result)?,
        _ => return Err(invalid()),
    }
    Ok(())
}

/// Retain every typed dependency of this space placement record.
fn edges_space_placement(
    value: &super::SpacePlacementData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(
        result,
        &value.source,
        Edge::Scalar(ScalarEdge::PlacementSource),
    )?;
    push(
        result,
        &value.ambient,
        Edge::Scalar(ScalarEdge::PlacementAmbient),
    )?;
    Ok(())
}

/// Retain every typed dependency of this scalar piece record.
fn edges_scalar_piece(
    value: &super::ScalarPieceData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(
        result,
        &value.placement,
        Edge::Scalar(ScalarEdge::PlacementDefinition),
    )?;
    push(result, &value.region, Edge::ParameterRegion)?;
    push(
        result,
        &value.local_frame,
        Edge::Scalar(ScalarEdge::LocalFrame),
    )?;
    Ok(())
}

/// Retain every typed dependency of this parameter base record.
fn edges_parameter_base(
    value: &super::ParameterBaseData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(result, &value.header, Edge::InHeader)?;
    push(result, &value.coordinates, Edge::Coordinates)?;
    push(result, &value.domain, Edge::SpaceDomain)?;
    Ok(())
}

/// Retain every typed dependency of this space family record.
fn edges_space_family(
    value: &super::SpaceFamilyData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(result, &value.base, Edge::Scalar(ScalarEdge::Base))?;
    push(
        result,
        &value.fibre_space,
        Edge::Scalar(ScalarEdge::FibreSpace),
    )?;
    push(result, &value.coefficients, Edge::CoefficientCoordinates)?;
    push(result, &value.unknowns, Edge::UnknownCoordinates)?;
    Ok(())
}

/// Retain every typed dependency of this fibre piece record.
fn edges_fibre_piece(
    value: &super::FibrePieceData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(result, &value.family, Edge::Scalar(ScalarEdge::Family))?;
    push(
        result,
        &value.stratum,
        Edge::Scalar(ScalarEdge::BaseStratum),
    )?;
    push(result, &value.region, Edge::ParameterRegion)?;
    push(result, &value.problem, Edge::Problem)?;
    push(result, &value.set, Edge::SolutionDomain)?;
    Ok(())
}

/// Retain every typed dependency of this real cell record.
fn edges_real_cell(
    value: &super::RealCellData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(result, &value.header, Edge::InHeader)?;
    push(result, &value.region, Edge::ParameterRegion)?;
    for (endpoint, role) in [
        (&value.left, ScalarEdge::LeftEndpoint),
        (&value.right, ScalarEdge::RightEndpoint),
    ] {
        if let RealEndpoint::RealExpression(endpoint) = endpoint {
            push(result, endpoint, Edge::Scalar(role))?;
        }
    }
    Ok(())
}

/// Retain every typed dependency of this family expression record.
fn edges_family_expression(
    value: &super::FamilyExpressionData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(result, &value.family, Edge::Scalar(ScalarEdge::Family))?;
    push(
        result,
        &value.ambient,
        Edge::Scalar(ScalarEdge::CheckedAmbient),
    )?;
    push(result, &value.region, Edge::ParameterRegion)?;
    for (position, argument) in value.arguments.iter().enumerate() {
        let position = crate::graph::position_u32(position)?;
        let role = match argument.role {
            FamilyArgumentRole::Base => ScalarEdge::BaseArgument(position),
            FamilyArgumentRole::Fibre => ScalarEdge::FibreArgument(position),
            FamilyArgumentRole::Family => ScalarEdge::FamilyArgument(position),
        };
        push(result, &argument.value, Edge::Scalar(role))?;
    }
    Ok(())
}

/// Retain every typed dependency of this coefficient evaluation record.
fn edges_coefficient_evaluation(
    value: &super::CoefficientEvaluationData<Handle>,
    result: &mut Vec<(LogicalId, Edge)>,
) -> Result<(), Fault> {
    push(result, &value.source, Edge::MapSource)?;
    push(result, &value.region, Edge::ParameterRegion)?;
    match &value.target {
        EvaluationTarget::Scalar { header, space } => {
            push(result, header, Edge::InHeader)?;
            push(result, space, Edge::Scalar(ScalarEdge::CheckedAmbient))?;
        }
        EvaluationTarget::Family { family } => {
            push(result, family, Edge::Scalar(ScalarEdge::Family))?;
        }
    }
    for binding in &value.bindings {
        push(
            result,
            &binding.image,
            Edge::Scalar(ScalarEdge::EvaluationSlot(binding.slot)),
        )?;
    }
    for (position, polynomial) in value.localization.iter().enumerate() {
        push(
            result,
            polynomial,
            Edge::Scalar(ScalarEdge::Localization(crate::graph::position_u32(
                position,
            )?)),
        )?;
    }
    Ok(())
}

/// Scoped bodies inherit an actual original header, never a synthetic retyped one.
pub(crate) fn metadata<'a>(
    state: &'a State,
    value: &'a SolveDraft<Handle>,
) -> Result<Option<(LogicalId, &'a Scope)>, Fault> {
    static EMPTY: Scope = Vec::new();
    let region = match value {
        SolveDraft::ScalarSpace(_) | SolveDraft::SpacePlacement(_) => return Ok(None),
        SolveDraft::ParameterBase(value) => return Ok(Some((value.header.logical_id(), &EMPTY))),
        SolveDraft::SpaceFamily(value) => {
            return Ok(Some((
                base(state, &value.base)?.header.logical_id(),
                &EMPTY,
            )));
        }
        SolveDraft::FamilyExpression(value) => {
            let shape = family(state, &value.family)?;
            return Ok(Some((
                base(state, &shape.base)?.header.logical_id(),
                &value.scope,
            )));
        }
        SolveDraft::RealCell(value) => return Ok(Some((value.header.logical_id(), &value.scope))),
        SolveDraft::ScalarPiece(value) => &value.region,
        SolveDraft::FibrePiece(value) => &value.region,
        SolveDraft::CoefficientEvaluation(value) => &value.region,
        _ => return Err(invalid()),
    };
    let region = state.region(region.logical_id())?;
    Ok(Some((region.header, &region.scope)))
}

/// These category checks admit propositions but confer no mathematical evidence.
pub(crate) fn statement(
    cas: &Cas,
    value: &SolveStatement,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(1, super::admission::AT)?;
    let state = &cas.state;
    match value {
        SolveStatement::CoefficientEvaluation(value) => {
            let SolveDraft::CoefficientEvaluation(evaluation) = object(state, &value.evaluation)?
            else {
                return Err(invalid());
            };
            if evaluation.region != value.region
                || value.left.logical_id().kind != VertexKind::Polynomial
            {
                return Err(invalid());
            }
            crate::descriptor::metadata(state, value.right.logical_id())?;
            Ok(())
        }
        SolveStatement::RealSign(value) => {
            let term = state.expression(value.subject.logical_id())?;
            let region = state.region(value.region.logical_id())?;
            if term.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
                || term.header != region.header
                || term.scope != region.scope
            {
                return Err(invalid());
            }
            Ok(())
        }
        SolveStatement::RealRootCount(value) => {
            relation(state, &value.relation, &value.region)?;
            if value.cells.len() != value.counts.len() {
                return Err(invalid());
            }
            for handle in &value.cells {
                if cell(state, handle)?.region != value.region {
                    return Err(invalid());
                }
            }
            Ok(())
        }
        SolveStatement::RealCellExists(value) => {
            relation(state, &value.relation, &value.region)?;
            if cell(state, &value.cell)?.region != value.region {
                return Err(invalid());
            }
            crate::domain_admission::predicate_context(state, value.predicate.logical_id())?;
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Require a Real root relation on the exact stated coefficient region.
fn relation(state: &State, handle: &Handle, region: &Handle) -> Result<(), Fault> {
    let SolveDraft::RootRelation(value) = object(state, handle)? else {
        return Err(invalid());
    };
    if value.ambient != Field::Real || value.region != *region {
        return Err(invalid());
    }
    Ok(())
}
