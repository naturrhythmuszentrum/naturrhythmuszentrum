// SPDX-License-Identifier: AGPL-3.0-only
//! Ordered original and captured witnesses are paired at every authentication boundary.

use super::{
    ApplicationError, BinaryRegion, Evidence, LoanError, PointwiseBinaryOperation, Side,
    same_symbols,
};
use crate::loan::{
    charge,
    registry::{Bundle, Declaration, Identity},
};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Original host incidence is logical identity despite distinct native layout occupations.
pub(super) fn source(
    bundle: &Bundle,
    axis: crate::CellAxis,
    extent: crate::Extent,
) -> Result<(), LoanError> {
    if bundle.origin.evidence.is_some() {
        return Err(LoanError::UnsupportedContract);
    }
    if bundle.declaration.coverage != crate::Coverage::RepresentedInputs {
        return Err(LoanError::CoverageMismatch);
    }
    if bundle.declaration.axis != axis
        || bundle.declaration.extent != extent
        || bundle.layout.gather.is_some()
        || bundle.layout.axis != axis
        || bundle.layout.components.extent() != extent
        || !bundle.layout.components.writable_affine()?
    {
        return Err(LoanError::LayoutMismatch);
    }
    Ok(())
}

/// Full contract and structure checks occur before demand or private output allocation.
pub(in crate::loan::application) fn check_request<P: Scalar + 'static>(
    heap: &crate::Heap,
    request: &super::super::prepared::Request<'_, P>,
    account: u64,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    charge(budget, 1)?;
    let (original, right) = request.binary.ok_or(LoanError::WrongLoan)?;
    let left = super::super::contract::source(request)?;
    let right_bundle = super::bundle(right)?;
    if pin_account(request.input)? != account
        || pin_account(right)? != account
        || request.destination.account != account
    {
        return Err(LoanError::ForeignAccount.into());
    }
    heap.current_bundle(request.input, budget)?;
    heap.current_bundle(right, budget)?;
    let member = request.member()?;
    let extent = request.destination.extent;
    let law = member.law();
    let inputs = [left.declaration.extent, right_bundle.declaration.extent];
    original.geometry().extents(inputs, extent)?;
    if law.inputs.as_ref() != inputs
        || law.output != extent
        || law.work != member.invocation(extent.entries())?
        || extent.columns() == 0
    {
        return Err(LoanError::UnsupportedContract.into());
    }
    regions(request.operation, inputs, extent, budget)?;
    charge(budget, 8)?;
    for (index, bundle) in [left, right_bundle].into_iter().enumerate() {
        let source_extent = bundle.declaration.extent;
        let axis =
            original
                .geometry()
                .source_axis(index, bundle.declaration.axis, source_extent, extent);
        source(bundle, axis, source_extent)?;
        super::super::contract::attachments(bundle)?;
    }
    original.validate(
        &[
            (&left.declaration, left.identity),
            (&right_bundle.declaration, right_bundle.identity),
        ],
        &request.destination.symbols,
        request.destination.axis,
    )?;
    if request.destination.coverage != crate::Coverage::RepresentedInputs {
        return Err(LoanError::CoverageMismatch.into());
    }
    let channels =
        left.channels.iter().flatten().count() + right_bundle.channels.iter().flatten().count();
    check_demand(heap, request, original, channels, budget)?;
    let payload = extent
        .entries()
        .checked_mul(3)
        .ok_or(LoanError::CountOverflow)?;
    request
        .limits
        .check(2, member.invocation(extent.columns())?.max(payload))?;
    Ok(())
}

/// Authenticate the pin continuation account independently of the source creation account.
fn pin_account<P: Scalar>(pin: &crate::BundlePin<P>) -> Result<u64, LoanError> {
    Ok(pin
        .record
        .payload
        .as_ref()
        .ok_or(LoanError::WrongLoan)?
        .account)
}

impl PointwiseBinaryOperation {
    /// Captured occupations and declarations must match both original ordered operands.
    fn validate(
        &self,
        sources: &[(&Declaration, Identity); 2],
        destination: &nrz_bindings::BindingSymbols,
        axis: crate::CellAxis,
    ) -> Result<(), LoanError> {
        if self.axis != axis {
            return Err(LoanError::LayoutMismatch);
        }
        if !same_symbols(&self.destination, destination) {
            return Err(LoanError::WrongLoan);
        }
        for (index, (source, identity)) in sources.iter().copied().enumerate() {
            if self.identities[index] != identity
                || !same_symbols(&self.sources[index], &source.symbols)
                || source.extent != self.operation.inputs()[index].shape().extent()
                || source.axis
                    != self.geometry().source_axis(
                        index,
                        source.axis,
                        source.extent,
                        self.operation.outputs()[0].shape().extent(),
                    )
            {
                return Err(LoanError::WrongLoan);
            }
            super::super::contract::declarations_with_shape(
                source,
                destination,
                self.operation.inputs()[index].descriptor(),
                self.operation.outputs()[0].descriptor(),
                source.extent == self.operation.outputs()[0].shape().extent(),
            )?;
        }
        Ok(())
    }
}
impl Side {
    /// Validate immutable original pairing without resolving historical source occupations.
    pub(in crate::loan::application) fn validate(
        &self,
        evidence: &Evidence,
        destination: &Declaration,
    ) -> Result<(), LoanError> {
        let inputs = Rc::ptr_eq(&self.original.operation.inputs, &evidence.operation.inputs);
        let outputs = Rc::ptr_eq(
            &self.original.operation.outputs,
            &evidence.operation.outputs,
        );
        let crate::binding::NativeRecipe::Kernel(law) = &self.original.operation.recipe else {
            return Err(LoanError::WrongLoan);
        };
        // The closed 21-identity block gives the final side debit offset 20; another
        // same-receipt token cannot substitute at a detached or historical boundary.
        if !inputs
            || !outputs
            || !Rc::ptr_eq(law, &evidence.law)
            || self.token.receipt != evidence.destination.receipt
            || Some(self.token.key) != evidence.destination.receipt.checked_add(20)
        {
            return Err(LoanError::WrongLoan);
        }
        self.original.validate(
            &[
                (&evidence.source, evidence.source_identity),
                (&self.source, self.identity),
            ],
            &destination.symbols,
            destination.axis,
        )?;
        self.original.geometry().extents(
            [evidence.source.extent, self.source.extent],
            destination.extent,
        )?;
        if self.source.coverage != destination.coverage {
            return Err(LoanError::LayoutMismatch);
        }
        for (index, attachment) in self.attachments.iter().enumerate() {
            if attachment.is_some() != self.identity.channels[index].is_some()
                || attachment.is_some_and(|item| {
                    item.identity != self.identity || item.channel != crate::HostChannel::ALL[index]
                })
            {
                return Err(LoanError::CoverageMismatch);
            }
        }
        Ok(())
    }
    /// Both live pin identity and selected-reader authentication precede any numerical load.
    pub(in crate::loan::application) fn validate_pin<P: Scalar + 'static>(
        &self,
        account: u64,
    ) -> Result<(), LoanError> {
        let input = self.input.as_ref().ok_or(LoanError::WrongLoan)?;
        if input.ticket()?.identity != self.identity
            || input
                .record
                .payload
                .as_ref()
                .ok_or(LoanError::WrongLoan)?
                .account
                != account
        {
            return Err(LoanError::WrongLoan);
        }
        input.checked_reader::<P>()?;
        Ok(())
    }
}

/// Original operand rectangles are inspected independently of the destination rectangle.
fn regions(
    operation: &crate::CheckedOperation,
    inputs: [crate::Extent; 2],
    extent: crate::Extent,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    for (position, region) in [
        BinaryRegion::Left,
        BinaryRegion::Right,
        BinaryRegion::Output,
    ]
    .into_iter()
    .zip(operation.inputs().iter().chain(operation.outputs()))
    {
        let expected = match position {
            BinaryRegion::Left => inputs[0],
            BinaryRegion::Right => inputs[1],
            BinaryRegion::Output => extent,
        };
        if region.shape().extent() != expected {
            return Err(LoanError::LayoutMismatch.into());
        }
        for (entry, contract) in region.shape().entries().iter().enumerate() {
            charge(budget, 1)?;
            if contract.is_zero() {
                return Err(ApplicationError::UnsupportedBinaryStructure {
                    position,
                    entry,
                    scope: region.shape().structure_scope(),
                });
            }
        }
    }
    Ok(())
}

/// Demand authenticates the exact captured layout before any factor-coordinate enumeration.
fn check_demand<P: Scalar>(
    heap: &crate::Heap,
    request: &super::super::prepared::Request<'_, P>,
    original: &PointwiseBinaryOperation,
    channels: usize,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    let extent = request.destination.extent;
    charge(budget, channels + 6)?;
    let demand = heap
        .mask(request.demand, original.demand_layout())
        .map_err(|cause| match cause {
            crate::MaskError::LayoutMismatch => LoanError::LayoutMismatch,
            other => LoanError::Mask(other),
        })?;
    if matches!(original.geometry(), super::Geometry::Cartesian(..)) {
        charge(budget, 4)?;
        if demand.gather().is_some() {
            return Err(LoanError::UnsupportedBinaryProjection.into());
        }
        if demand.axis() != original.axis || demand.components().extent() != extent {
            return Err(LoanError::LayoutMismatch.into());
        }
    }
    if demand.words().len() != crate::loan::words(extent.rows()) {
        return Err(LoanError::CoverageMismatch.into());
    }
    let partial = extent.rows() % 64;
    if partial != 0
        && demand
            .words()
            .last()
            .is_some_and(|word| *word >> partial != 0)
    {
        return Err(LoanError::CoverageMismatch.into());
    }
    Ok(())
}
