// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit operation intent is validated before constructing distribution syntax.

use crate::budget::{Budget, BudgetKind};
use crate::distribution_syntax::{AT, invalid};
use crate::domain_model::Layer;
use crate::error::{Fault, FaultKind, InvalidCode};
use crate::graph::{Cas, Expression, LogicalId};
use crate::model::Sort;
use crate::request_model::Request;
use crate::task::{ActionMode, Task};

/// Point values, derivative coordinates and products retain distinct failure meanings.
pub(crate) fn check(
    cas: &mut Cas,
    request: &Request,
    inputs: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let input = cas
        .state
        .expression(*inputs.first().ok_or_else(invalid)?)?
        .clone();
    match &request.task {
        Task::ApplyAt { .. } if input.sort == Sort::Distribution => {
            return Err(Fault::invalid(InvalidCode::DistributionPointValue, AT));
        }
        Task::Differentiate {
            coordinate,
            domain,
            layer: Layer::Distribution,
            order,
        } => {
            if input.sort != Sort::Distribution || *order == 0 {
                return Err(invalid());
            }
            let definition = cas.state.space(input.space.ok_or_else(invalid)?)?;
            if definition.coordinate != *coordinate || definition.domain != domain.logical_id() {
                return Err(invalid());
            }
            budget.check(BudgetKind::Depth, u64::from(*order), AT)?;
        }
        _ => operation(cas, &request.task, inputs, &input)?,
    }
    if matches!(
        request.task,
        Task::SmoothMultiply { .. } | Task::Product { .. }
    ) {
        crate::distribution_refusal::multiplier(cas, inputs, budget).map_err(|mut fault| {
            if crate::case_obligations::unknown(&fault) {
                fault.kind = FaultKind::Invalid(InvalidCode::PendingPremise);
            }
            fault
        })?;
    }
    Ok(())
}

/// Exact interval and space identity are independent of constructor sort compatibility.
fn operation(
    cas: &Cas,
    task: &Task,
    inputs: &[LogicalId],
    input: &Expression,
) -> Result<(), Fault> {
    if matches!(task, Task::Product { .. } | Task::SmoothMultiply { .. })
        && input.sort == Sort::Distribution
        && cas
            .state
            .expression(*inputs.get(1).ok_or_else(invalid)?)?
            .sort
            == Sort::Distribution
    {
        return Err(Fault::invalid(
            InvalidCode::DistributionProductUndefined,
            AT,
        ));
    }
    match task {
        Task::Equal { domain, .. } if input.space.is_some() => {
            if cas.state.space(input.space.ok_or_else(invalid)?)?.domain != domain.logical_id() {
                return Err(invalid());
            }
        }
        Task::SmoothMultiply { space } => {
            if !inputs.iter().any(|id| {
                cas.state.expression(*id).is_ok_and(|value| {
                    value.sort == Sort::Distribution && value.space == Some(space.logical_id())
                })
            }) {
                return Err(invalid());
            }
        }
        Task::Product { requested_sort } => {
            let right = cas.state.expression(*inputs.get(1).ok_or_else(invalid)?)?;
            if (input.sort == Sort::Distribution || right.sort == Sort::Distribution)
                && *requested_sort != Sort::Distribution
            {
                return Err(invalid());
            }
        }
        Task::Pair { space, action_mode } => {
            let function = cas.state.expression(*inputs.get(1).ok_or_else(invalid)?)?;
            let ordinary = *action_mode == ActionMode::Test;
            if input.sort != Sort::Distribution
                || input.space != Some(space.logical_id())
                || (ordinary
                    && (function.sort != Sort::Test || function.space != Some(space.logical_id())))
                || (!ordinary && function.space.is_some())
            {
                return Err(Fault::invalid(InvalidCode::TestSpaceMismatch, AT));
            }
        }
        _ => {}
    }
    Ok(())
}
