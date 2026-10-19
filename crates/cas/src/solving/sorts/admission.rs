// SPDX-License-Identifier: AGPL-3.0-only
//! Structural validation supplies no membership, order, unit or existence theorem.

use super::model::{
    CoefficientEvaluationData, EvaluationTarget, FamilyArgument, FamilyArgumentRole,
    FamilyExpressionData, FibrePieceData, ParameterBaseData, RealCellData, RealEndpoint,
    SpaceFamilyData,
};
use crate::budget::{Budget, BudgetKind};
use crate::domain_model::AmbientDomain;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Handle, State, Vertex, size};
use crate::model::{AtomicSort, Field, Operator, Sort};
use crate::solving::{CoordinateRole, SolveDraft};

/// Location for bounded scalar-space validation failures.
pub(super) const AT: Location = Location::new("scalar family admission");
/// Reject a contradictory scalar role or declared sort.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(InvalidCode::SymbolSortMismatch, AT)
}
/// Borrow an earlier typed solving object.
pub(super) fn object<'a>(
    state: &'a State,
    value: &Handle,
) -> Result<&'a SolveDraft<Handle>, Fault> {
    crate::solving::graph::object(state, value.logical_id())
}

/// Require an earlier original Real parameter base.
pub(super) fn base<'a>(
    state: &'a State,
    value: &Handle,
) -> Result<&'a ParameterBaseData<Handle>, Fault> {
    match object(state, value)? {
        SolveDraft::ParameterBase(value) => Ok(value),
        _ => Err(invalid()),
    }
}
/// Require an earlier exact space family.
pub(super) fn family<'a>(
    state: &'a State,
    value: &Handle,
) -> Result<&'a SpaceFamilyData<Handle>, Fault> {
    match object(state, value)? {
        SolveDraft::SpaceFamily(value) => Ok(value),
        _ => Err(invalid()),
    }
}
/// Require earlier structural Real cell syntax.
pub(super) fn cell<'a>(
    state: &'a State,
    value: &Handle,
) -> Result<&'a RealCellData<Handle>, Fault> {
    match object(state, value)? {
        SolveDraft::RealCell(value) => Ok(value),
        _ => Err(invalid()),
    }
}

/// Validate the new syntax while preserving all original operand references.
pub(crate) fn validate(
    state: &State,
    value: &SolveDraft<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    budget.work(size(value.objects().len(), AT)?, AT)?;
    match value {
        SolveDraft::ScalarSpace(value) => super::registry::validate(value).map(|_| ()),
        SolveDraft::SpacePlacement(value) => super::registry::placement(state, value),
        SolveDraft::ScalarPiece(value) => super::incidence::validate(state, value),
        SolveDraft::ParameterBase(value) => check_base(state, value),
        SolveDraft::SpaceFamily(value) => check_family(state, value),
        SolveDraft::FamilyExpression(value) => check_expression(state, value, budget),
        SolveDraft::FibrePiece(value) => check_piece(state, value),
        SolveDraft::RealCell(value) => check_cell(state, value, budget),
        SolveDraft::CoefficientEvaluation(value) => check_evaluation(state, value, budget),
        _ => Err(invalid()),
    }
}

/// Every base coordinate remains an original Real coefficient.
fn check_base(state: &State, value: &ParameterBaseData<Handle>) -> Result<(), Fault> {
    let frame = crate::solving::graph::coordinates(state, value.coordinates.logical_id())?;
    let region = state.region(value.domain.logical_id())?;
    if frame.header != value.header
        || region.header != value.header.logical_id()
        || !region.scope.is_empty()
        || region.lies_over.is_some()
        || region.ambient
            != (AmbientDomain::Product {
                coordinates: value.coordinates.clone(),
            })
        || frame
            .coordinates
            .iter()
            .any(|entry| entry.field != Field::Real || entry.role != CoordinateRole::Coefficient)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Family coordinates are disjoint original declarations in their exact local roles.
fn check_family(state: &State, value: &SpaceFamilyData<Handle>) -> Result<(), Fault> {
    let original = base(state, &value.base)?;
    let fibre = super::field(state, &value.fibre_space)?;
    let coefficients = crate::solving::graph::coordinates(state, value.coefficients.logical_id())?;
    let unknowns = crate::solving::graph::coordinates(state, value.unknowns.logical_id())?;
    let parameters = crate::solving::graph::coordinates(state, original.coordinates.logical_id())?;
    if coefficients.header != original.header || unknowns.header != original.header {
        return Err(invalid());
    }
    for (frame, role) in [
        (coefficients, CoordinateRole::Coefficient),
        (unknowns, CoordinateRole::Unknown),
    ] {
        if frame.coordinates.iter().any(|entry| {
            entry.field != fibre
                || entry.role != role
                || parameters
                    .coordinates
                    .iter()
                    .any(|parameter| parameter.key == entry.key)
        }) {
            return Err(invalid());
        }
    }
    if coefficients.coordinates.iter().any(|entry| {
        unknowns
            .coordinates
            .iter()
            .any(|unknown| unknown.key == entry.key)
    }) {
        return Err(invalid());
    }
    Ok(())
}

/// The exact projection is structural, with membership left to ordinary region evidence.
pub(super) fn projection_matches(
    state: &State,
    region: &Handle,
    family: &Handle,
    stratum: Option<&Handle>,
) -> Result<(), Fault> {
    let region = state.region(region.logical_id())?;
    let projection = region.lies_over.as_ref().ok_or_else(invalid)?;
    if projection.family != family.logical_id()
        || stratum.is_some_and(|value| value.logical_id() != projection.stratum)
    {
        return Err(invalid());
    }
    Ok(())
}

/// Validate a local operator body against its family, region and argument roles.
fn check_expression(
    state: &State,
    value: &FamilyExpressionData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    let shape = family(state, &value.family)?;
    let original = base(state, &shape.base)?;
    let region = state.region(value.region.logical_id())?;
    projection_matches(state, &value.region, &value.family, None)?;
    if region.header != original.header.logical_id() || region.scope != value.scope {
        return Err(invalid());
    }
    let output = super::field(state, &value.ambient)?;
    let component = matches!(value.operator, Operator::RealPart | Operator::ImagPart);
    let fibre = super::field(state, &shape.fibre_space)?;
    let complex_operator = component || matches!(value.operator, Operator::Conjugate);
    if complex_operator && fibre != Field::Complex {
        return Err(Fault::invalid(
            InvalidCode::UnsupportedFragment,
            Location::new("family complex operator signature"),
        ));
    }
    let expected = if component { Field::Real } else { fibre };
    if value.result != Sort::Value(AtomicSort::Scalar(expected.clone())) {
        return Err(invalid());
    }
    if output != expected {
        return Err(Fault::invalid(InvalidCode::AmbientSpaceMismatch, AT));
    }
    signature(&value.operator, value.arguments.len())?;
    for argument in &value.arguments {
        check_argument(state, value, shape, argument, budget)?;
    }
    Ok(())
}

/// Check the closed local operator arity without assuming a theorem.
const fn signature(operator: &Operator, count: usize) -> Result<(), Fault> {
    let arity = match operator {
        Operator::Neg
        | Operator::IntegerPower(_)
        | Operator::Conjugate
        | Operator::RealPart
        | Operator::ImagPart => 1,
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div => 2,
        _ => {
            return Err(Fault::invalid(
                InvalidCode::UnsupportedFragment,
                Location::new("family operator signature"),
            ));
        }
    };
    if count != arity {
        return Err(invalid());
    }
    Ok(())
}

/// Check each original argument against its exact base or fibre frame.
fn check_argument(
    state: &State,
    value: &FamilyExpressionData<Handle>,
    shape: &SpaceFamilyData<Handle>,
    argument: &FamilyArgument<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    if argument.role == FamilyArgumentRole::Family {
        let SolveDraft::FamilyExpression(previous) = object(state, &argument.value)? else {
            return Err(invalid());
        };
        if previous.family != value.family
            || previous.region != value.region
            || previous.scope != value.scope
        {
            return Err(invalid());
        }
        return Ok(());
    }
    let expression = state.expression(argument.value.logical_id())?;
    let expected = if argument.role == FamilyArgumentRole::Base {
        Field::Real
    } else {
        super::field(state, &shape.fibre_space)?
    };
    let region = state.region(value.region.logical_id())?;
    if expression.sort != Sort::Value(AtomicSort::Scalar(expected))
        || expression.header != region.header
        || expression.scope != value.scope
    {
        return Err(invalid());
    }
    let original = base(state, &shape.base)?;
    let frame = if argument.role == FamilyArgumentRole::Base {
        &original.coordinates
    } else {
        &shape.coefficients
    };
    let frame = crate::solving::graph::coordinates(state, frame.logical_id())?;
    budget.work(
        size(frame.coordinates.len(), AT)?.saturating_mul(budget.limits().name_bytes),
        AT,
    )?;
    let mut keys: std::collections::BTreeSet<_> = frame
        .coordinates
        .iter()
        .map(|entry| entry.key.as_str())
        .collect();
    if argument.role == FamilyArgumentRole::Fibre {
        let unknowns = crate::solving::graph::coordinates(state, shape.unknowns.logical_id())?;
        budget.work(
            size(unknowns.coordinates.len(), AT)?.saturating_mul(budget.limits().name_bytes),
            AT,
        )?;
        keys.extend(unknowns.coordinates.iter().map(|entry| entry.key.as_str()));
    }
    crate::solving::graph::free_keys(state, &argument.value, &keys, true, budget)?;
    Ok(())
}

/// Keep a proposed solution set attached to its exact family problem and stratum.
fn check_piece(state: &State, value: &FibrePieceData<Handle>) -> Result<(), Fault> {
    let shape = family(state, &value.family)?;
    projection_matches(state, &value.region, &value.family, Some(&value.stratum))?;
    let problem = crate::solving::graph::problem(state, value.problem.logical_id())?;
    let SolveDraft::SolutionSet(set) = object(state, &value.set)? else {
        return Err(invalid());
    };
    if set.problem != value.problem
        || problem.region != value.region
        || problem.unknowns != shape.unknowns
        || problem.ambient != super::field(state, &shape.fibre_space)?
    {
        return Err(invalid());
    }
    Ok(())
}

/// Check endpoint sorts and infinity openness without asserting endpoint order.
fn check_cell(
    state: &State,
    value: &RealCellData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    crate::validation::scope_limit(&value.scope, budget)?;
    let region = state.region(value.region.logical_id())?;
    if region.header != value.header.logical_id() || region.scope != value.scope {
        return Err(invalid());
    }
    let left_invalid = matches!(value.left, RealEndpoint::PositiveInfinity)
        || matches!(value.left, RealEndpoint::NegativeInfinity) && !value.left_open;
    let right_invalid = matches!(value.right, RealEndpoint::NegativeInfinity)
        || matches!(value.right, RealEndpoint::PositiveInfinity) && !value.right_open;
    if left_invalid || right_invalid {
        return Err(invalid());
    }
    for endpoint in [&value.left, &value.right] {
        if let RealEndpoint::RealExpression(endpoint) = endpoint {
            let term = state.expression(endpoint.logical_id())?;
            if term.sort != Sort::Value(AtomicSort::Scalar(Field::Real))
                || term.header != region.header
                || term.scope != value.scope
            {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

/// Validate complete ordered slot bindings and finite localization syntax.
pub(super) fn check_evaluation(
    state: &State,
    value: &CoefficientEvaluationData<Handle>,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let source = crate::solving::graph::coordinates(state, value.source.logical_id())?;
    let region = state.region(value.region.logical_id())?;
    if value.bindings.len() != source.coordinates.len()
        || source
            .coordinates
            .iter()
            .any(|slot| slot.role != CoordinateRole::Coefficient)
    {
        return Err(invalid());
    }
    match &value.target {
        EvaluationTarget::Scalar { header, space } => {
            let target = super::field(state, space)?;
            if header.logical_id() != region.header
                || source.coordinates.iter().any(|slot| slot.field != target)
            {
                return Err(invalid());
            }
        }
        EvaluationTarget::Family { family } => {
            projection_matches(state, &value.region, family, None)?;
        }
    }
    for (position, (binding, slot)) in value.bindings.iter().zip(&source.coordinates).enumerate() {
        budget.work(1, AT)?;
        if binding.slot != crate::graph::position_u32(position)? {
            return Err(invalid());
        }
        evaluation_image(state, value, &binding.image, &slot.field)?;
        evaluation_support(state, value, &binding.image, &slot.field, budget)?;
    }
    for polynomial in &value.localization {
        let SolveDraft::Polynomial(polynomial) = object(state, polynomial)? else {
            return Err(invalid());
        };
        if !polynomial
            .variables
            .iter()
            .eq(source.coordinates.iter().map(|slot| &slot.field))
        {
            return Err(invalid());
        }
    }
    budget.check(BudgetKind::Edges, size(value.localization.len(), AT)?, AT)
}

/// Require each evaluation image in the exact original slot sort and target region.
fn evaluation_image(
    state: &State,
    value: &CoefficientEvaluationData<Handle>,
    image: &Handle,
    field: &Field,
) -> Result<(), Fault> {
    let region = state.region(value.region.logical_id())?;
    match state.node(image.logical_id())? {
        Vertex::Expression(expression)
            if expression.sort == Sort::Value(AtomicSort::Scalar(field.clone()))
                && expression.header == region.header
                && expression.scope == region.scope =>
        {
            Ok(())
        }
        Vertex::Solve(body) => {
            let (EvaluationTarget::Family { family }, SolveDraft::FamilyExpression(expression)) =
                (&value.target, body.as_ref())
            else {
                return Err(invalid());
            };
            if expression.family != *family
                || expression.region != value.region
                || expression.result != Sort::Value(AtomicSort::Scalar(field.clone()))
            {
                return Err(invalid());
            }
            Ok(())
        }
        _ => Err(invalid()),
    }
}

/// Symbolic cell selection is independent of the retained rational interval contract.
pub(crate) fn selection_cell(
    state: &State,
    value: &crate::solving::RootSelectionData<Handle>,
) -> Result<(), Fault> {
    let Some(handle) = &value.cell else {
        return Ok(());
    };
    let cell = cell(state, handle)?;
    let [relation] = value.relations.as_slice() else {
        return Err(invalid());
    };
    let SolveDraft::RootRelation(relation) = object(state, relation)? else {
        return Err(invalid());
    };
    if relation.ambient != Field::Real
        || relation.region != cell.region
        || cell.header != value.header
        || cell.scope != value.scope
    {
        return Err(invalid());
    }
    Ok(())
}

/// Family coefficient images depend on the exact original coefficient roles, never unknowns.
fn evaluation_support(
    state: &State,
    value: &CoefficientEvaluationData<Handle>,
    image: &Handle,
    field: &Field,
    budget: &mut Budget,
) -> Result<(), Fault> {
    let EvaluationTarget::Family { family: handle } = &value.target else {
        return Ok(());
    };
    let shape = family(state, handle)?;
    let original = base(state, &shape.base)?;
    let base_frame = crate::solving::graph::coordinates(state, original.coordinates.logical_id())?;
    let fibre_frame = crate::solving::graph::coordinates(state, shape.coefficients.logical_id())?;
    let count = base_frame
        .coordinates
        .len()
        .saturating_add(fibre_frame.coordinates.len());
    budget.work(
        size(count, AT)?.saturating_mul(budget.limits().name_bytes),
        AT,
    )?;
    let mut keys = std::collections::BTreeSet::new();
    let local_body = matches!(state.node(image.logical_id())?, Vertex::Solve(_));
    if *field == Field::Real || local_body {
        keys.extend(base_frame.coordinates.iter().map(|slot| slot.key.as_str()));
    }
    if *field == super::field(state, &shape.fibre_space)? {
        keys.extend(fibre_frame.coordinates.iter().map(|slot| slot.key.as_str()));
    }
    crate::solving::graph::free_keys(state, image, &keys, true, budget)
}
