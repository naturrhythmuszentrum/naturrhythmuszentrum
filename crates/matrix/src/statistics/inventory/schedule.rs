// SPDX-License-Identifier: AGPL-3.0-only
//! Original operand metadata and closed laws determine every finite statistical request.

use super::{
    Channel, Role,
    plan::{Specification, State},
};
use crate::statistics::capture::InputBinding;
use crate::{CheckedOperation, Extent, OperationError};
use nrz_scalar::Precision;
use std::cell::Cell;

/// Captured inputs and all numerical checks are planned before the first parameter read.
pub(super) fn capture(
    bindings: &[InputBinding],
    operation: Option<&CheckedOperation>,
    requests: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    for (slot, input) in bindings.iter().enumerate() {
        let precision = input.dependency.precision();
        pair(
            requests,
            Role::InputSensitivity,
            slot,
            Extent::new(input.rows.len(), input.lineage.columns.len())?,
            precision,
            false,
        );
        if operation.is_none() {
            continue;
        }
        if input.means {
            pair(
                requests,
                Role::InputMeans,
                slot,
                Extent::new(input.rows.len(), 1)?,
                precision,
                false,
            );
        }
        pair(
            requests,
            Role::OperatingPoint,
            slot,
            input.dependency.layout().extent(),
            precision,
            false,
        );
    }
    if let Some(operation) = operation {
        checks(bindings, operation, requests)?;
    }
    Ok(())
}
/// Structural and fixed-operand checks retain their own original model covariance roles.
fn checks(
    bindings: &[InputBinding],
    operation: &CheckedOperation,
    requests: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    for (slot, (input, contract)) in bindings.iter().zip(operation.inputs()).enumerate() {
        let extent = Extent::new(input.lineage.columns.len(), input.lineage.columns.len())?;
        let structural = contract.shape().structure_scope() == crate::StructureScope::WholeFamily
            && contract
                .shape()
                .entries()
                .iter()
                .any(crate::EntryContract::is_zero);
        if structural {
            pair(
                requests,
                Role::StructureCovariance,
                slot,
                extent,
                input.dependency.precision(),
                false,
            );
        }
        let fixed = match &operation.recipe {
            crate::binding::NativeRecipe::Solve(_) => slot == 0,
            crate::binding::NativeRecipe::Kernel(kernel) => {
                kernel.kind() == crate::KernelKind::Product
            }
            _ => false,
        };
        if fixed {
            pair(
                requests,
                Role::FixedCovariance,
                slot,
                extent,
                input.dependency.precision(),
                true,
            );
        }
    }
    Ok(())
}
/// Generic evaluation owns its own finite schedule; solve contractions retain their existing plan.
pub(super) fn evaluate(
    bindings: &[InputBinding],
    operation: &CheckedOperation,
    sources: usize,
    precision: Precision,
    requests: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    if matches!(operation.recipe, crate::binding::NativeRecipe::Solve(_)) {
        return Ok(());
    }
    let inputs = bindings.iter().try_fold(0_usize, |sum, input| {
        sum.checked_add(input.rows.len())
            .ok_or(OperationError::ResourceLimit)
    })?;
    pair(
        requests,
        Role::Inputs,
        0,
        Extent::new(inputs, sources)?,
        precision,
        false,
    );
    pair(
        requests,
        Role::Model,
        0,
        Extent::new(sources, sources)?,
        precision,
        false,
    );
    let means = bindings.iter().all(|input| input.means);
    if means {
        pair(
            requests,
            Role::Means,
            0,
            Extent::new(inputs, 1)?,
            precision,
            true,
        );
    }
    for (slot, output) in operation.outputs().iter().enumerate() {
        output_schedule(
            requests,
            slot,
            [output.shape().extent().entries(), inputs, sources],
            precision,
            means,
        )?;
    }
    Ok(())
}
/// Each output has independently identified derivative, source, covariance and parameter owners.
fn output_schedule(
    requests: &mut Vec<Specification>,
    slot: usize,
    dimensions: [usize; 3],
    precision: Precision,
    means: bool,
) -> Result<(), OperationError> {
    let [rows, inputs, sources] = dimensions;
    let jacobian = Extent::new(rows, inputs)?;
    let sensitivity = Extent::new(rows, sources)?;
    let covariance = Extent::new(rows, rows)?;
    pair(requests, Role::Jacobian, slot, jacobian, precision, false);
    pair(
        requests,
        Role::Offsets,
        slot,
        Extent::new(rows, 1)?,
        precision,
        false,
    );
    contraction(
        requests,
        Role::Sensitivity,
        slot,
        [jacobian, Extent::new(inputs, sources)?],
        precision,
        false,
    )?;
    contraction(
        requests,
        Role::CovarianceProduct,
        slot,
        [sensitivity, Extent::new(sources, sources)?],
        precision,
        false,
    )?;
    pair(
        requests,
        Role::Transpose,
        slot,
        Extent::new(sources, rows)?,
        precision,
        false,
    );
    contraction(
        requests,
        Role::Covariance,
        slot,
        [sensitivity, Extent::new(sources, rows)?],
        precision,
        false,
    )?;
    publication(
        requests,
        Role::PublishedCovariance,
        slot,
        covariance,
        precision,
        false,
    );
    publication(
        requests,
        Role::PublishedSensitivity,
        slot,
        sensitivity,
        precision,
        false,
    );
    publication(
        requests,
        Role::PublishedJacobian,
        slot,
        jacobian,
        precision,
        false,
    );
    if means {
        contraction(
            requests,
            Role::Mean,
            slot,
            [jacobian, Extent::new(inputs, 1)?],
            precision,
            true,
        )?;
        publication(
            requests,
            Role::PublishedMean,
            slot,
            Extent::new(rows, 1)?,
            precision,
            true,
        );
    }
    Ok(())
}
/// Standalone cross covariance plans both ordered target selections and complete source alignment.
pub(super) fn cross(
    bindings: &[InputBinding],
    sources: usize,
    precision: Precision,
    requests: &mut Vec<Specification>,
) -> Result<(), OperationError> {
    let left = bindings
        .first()
        .ok_or(crate::PrivateAllocationError::IncompleteCoverage)?
        .rows
        .len();
    let right = bindings
        .get(1)
        .ok_or(crate::PrivateAllocationError::IncompleteCoverage)?
        .rows
        .len();
    let first = Extent::new(left, sources)?;
    let model = Extent::new(sources, sources)?;
    pair(requests, Role::Model, 0, model, precision, false);
    pair(requests, Role::Aligned, 0, first, precision, false);
    contraction(
        requests,
        Role::CovarianceProduct,
        0,
        [first, model],
        precision,
        false,
    )?;
    let transposed = Extent::new(sources, right)?;
    pair(requests, Role::Transpose, 0, transposed, precision, false);
    contraction(
        requests,
        Role::Covariance,
        0,
        [first, transposed],
        precision,
        false,
    )?;
    publication(
        requests,
        Role::PublishedCovariance,
        0,
        Extent::new(left, right)?,
        precision,
        false,
    );
    Ok(())
}
/// Two independent vector owners represent one candidate matrix and its exact-target enclosure.
fn pair(
    requests: &mut Vec<Specification>,
    role: Role,
    index: usize,
    extent: Extent,
    precision: Precision,
    optional: bool,
) {
    for channel in [Channel::Centers, Channel::Truth] {
        requests.push(specification(
            role, index, channel, extent, precision, optional,
        ));
    }
}
/// Publication copies have distinct center and error roles, without an enclosure endpoint vector.
fn publication(
    requests: &mut Vec<Specification>,
    role: Role,
    index: usize,
    extent: Extent,
    precision: Precision,
    optional: bool,
) {
    for channel in [Channel::Centers, Channel::Errors] {
        requests.push(specification(
            role, index, channel, extent, precision, optional,
        ));
    }
}
/// Three opaque rectangles and two observable vectors precede every backend contraction.
fn contraction(
    requests: &mut Vec<Specification>,
    role: Role,
    index: usize,
    operands: [Extent; 2],
    precision: Precision,
    optional: bool,
) -> Result<(), OperationError> {
    let [left, right] = operands;
    if left.columns() != right.rows() {
        return Err(crate::StatisticsError::Contract.into());
    }
    let extent = Extent::new(left.rows(), right.columns())?;
    for (channel, shape) in [
        (Channel::BackendLeft, left),
        (Channel::BackendRight, right),
        (Channel::BackendResult, extent),
    ] {
        requests.push(specification(
            role, index, channel, shape, precision, optional,
        ));
    }
    pair(requests, role, index, extent, precision, optional);
    Ok(())
}
/// Every request is initially unavailable to numerical code until its typed permit is drawn.
const fn specification(
    role: Role,
    index: usize,
    channel: Channel,
    extent: Extent,
    precision: Precision,
    optional: bool,
) -> Specification {
    Specification {
        role,
        index,
        channel,
        extent,
        precision,
        optional,
        state: Cell::new(State::Planned),
        bytes: Cell::new(None),
    }
}

/// Exact request count reserves metadata once without spare schedule capacity or numerical allocation.
pub(super) fn count(
    bindings: &[InputBinding],
    operation: Option<&CheckedOperation>,
) -> Result<usize, OperationError> {
    let mut total = bindings
        .len()
        .checked_mul(2)
        .ok_or(OperationError::ResourceLimit)?;
    let Some(operation) = operation else {
        return total.checked_add(18).ok_or(OperationError::ResourceLimit);
    };
    for (slot, (input, contract)) in bindings.iter().zip(operation.inputs()).enumerate() {
        let structural = contract.shape().structure_scope() == crate::StructureScope::WholeFamily
            && contract
                .shape()
                .entries()
                .iter()
                .any(crate::EntryContract::is_zero);
        let fixed = match &operation.recipe {
            crate::binding::NativeRecipe::Solve(_) => slot == 0,
            crate::binding::NativeRecipe::Kernel(kernel) => {
                kernel.kind() == crate::KernelKind::Product
            }
            _ => false,
        };
        total = total
            .checked_add(
                2 * (1 + usize::from(input.means) + usize::from(structural) + usize::from(fixed)),
            )
            .ok_or(OperationError::ResourceLimit)?;
    }
    if matches!(operation.recipe, crate::binding::NativeRecipe::Solve(_)) {
        return Ok(total);
    }
    let means = bindings.iter().all(|input| input.means);
    operation
        .outputs()
        .len()
        .checked_mul(if means { 34 } else { 27 })
        .and_then(|n| n.checked_add(total)?.checked_add(if means { 6 } else { 4 }))
        .ok_or(OperationError::ResourceLimit)
}
