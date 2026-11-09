// SPDX-License-Identifier: AGPL-3.0-only
//! Original meanings and full-shape exclusion precede every numerical allocation.

use super::{ApplicationError, Evidence, PointwiseRequest};
use crate::loan::{
    HostChannel, LoanError, charge,
    registry::{Bundle, Declaration, Identity},
};
use crate::{
    Elementwise, Heap,
    binding::NativeRecipe,
    kernels::{KernelContract, contract::Recipe},
};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Borrow the exact source owner without creating another numerical edge.
pub(super) fn source<P: Scalar>(request: &PointwiseRequest<P>) -> Result<&Bundle, LoanError> {
    request
        .input
        .record
        .payload
        .as_ref()
        .and_then(|pin| pin.bundle.payload.as_ref())
        .ok_or(LoanError::WrongLoan)
}
/// Validate all immutable metadata and every original structural entry before allocation.
pub(super) fn check<P: Scalar + 'static>(
    heap: &Heap,
    request: &PointwiseRequest<P>,
    account: u64,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    charge(budget, 1)?;
    let pin = request
        .input
        .record
        .payload
        .as_ref()
        .ok_or(LoanError::WrongLoan)?;
    if pin.account != account || request.destination.account != account {
        return Err(LoanError::ForeignAccount.into());
    }
    heap.current_bundle(&request.input, budget)?;
    let bundle = source(request)?;
    if bundle.origin.evidence.is_some() {
        return Err(LoanError::UnsupportedContract.into());
    }
    let law = square(&request.operation)?;
    let destination = &request.destination;
    let extent = bundle.declaration.extent;
    if destination.extent != extent || extent.columns() == 0 {
        return Err(LoanError::UnsupportedContract.into());
    }
    if request.operation.inputs()[0].shape().extent() != extent
        || request.operation.outputs()[0].shape().extent() != extent
    {
        return Err(LoanError::LayoutMismatch.into());
    }
    if law.inputs.as_ref() != [extent] || law.output != extent {
        return Err(LoanError::UnsupportedContract.into());
    }
    // Exclusion deliberately precedes demand: sparse and empty selections cannot hide a zero.
    for (output, region) in [
        (false, &request.operation.inputs()[0]),
        (true, &request.operation.outputs()[0]),
    ] {
        for (entry, contract) in region.shape().entries().iter().enumerate() {
            charge(budget, 1)?;
            if contract.is_zero() {
                return Err(ApplicationError::UnsupportedStructure {
                    output,
                    entry,
                    scope: region.shape().structure_scope(),
                });
            }
        }
    }
    charge(budget, 4)?;
    if destination.axis != bundle.declaration.axis
        || bundle.layout.gather.is_some()
        || request.demand.layout != bundle.identity.layout
    {
        return Err(LoanError::LayoutMismatch.into());
    }
    if destination.coverage != bundle.declaration.coverage {
        return Err(LoanError::CoverageMismatch.into());
    }
    declarations(
        &bundle.declaration,
        &destination.symbols,
        &request.operation,
    )?;
    attachments(bundle)?;
    charge(budget, bundle.channels.iter().flatten().count() + 3)?;
    let mask = heap
        .mask(request.demand, bundle.identity.layout)
        .map_err(|cause| match cause {
            crate::MaskError::LayoutMismatch => LoanError::LayoutMismatch,
            other => LoanError::Mask(other),
        })?;
    let cells = extent.rows();
    let words = mask.words();
    if words.len() != crate::loan::words(cells)
        || (cells % 64 != 0 && words.last().is_some_and(|word| *word >> (cells % 64) != 0))
    {
        return Err(LoanError::CoverageMismatch.into());
    }
    let work = crate::kernels::contract::work(extent.columns(), 12)?;
    let payload = extent
        .entries()
        .checked_mul(3)
        .ok_or(LoanError::CountOverflow)?;
    request.limits.check(1, work.max(payload))?;
    Ok(())
}
/// Match the independently declared result convention, including exact fresh channel identities.
fn declarations(
    source: &Declaration,
    output: &nrz_bindings::BindingSymbols,
    operation: &crate::CheckedOperation,
) -> Result<(), LoanError> {
    source
        .symbols
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    output
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    if source.symbols.value().descriptor() != operation.inputs()[0].descriptor()
        || output.value().descriptor() != operation.outputs()[0].descriptor()
    {
        return Err(LoanError::UnsupportedContract);
    }
    if output.value().shape() != source.symbols.value().shape()
        || output.bound().is_none()
        || output.error().is_none()
    {
        return Err(LoanError::UnsupportedContract);
    }
    let inputs = [
        Some(source.symbols.value()),
        source.symbols.bound(),
        source.symbols.error(),
    ];
    for symbol in [Some(output.value()), output.bound(), output.error()]
        .into_iter()
        .flatten()
    {
        if inputs.into_iter().flatten().any(|input| input == symbol) {
            return Err(LoanError::TargetMismatch);
        }
    }
    for symbol in [output.bound(), output.error()].into_iter().flatten() {
        if symbol.target() != Some(output.value().node())
            || symbol.shape() != output.value().shape()
        {
            return Err(LoanError::TargetMismatch);
        }
    }
    Ok(())
}
/// Every filled role is authenticated even when all roles share one word owner.
fn attachments(bundle: &Bundle) -> Result<(), LoanError> {
    for (channel, owner) in HostChannel::ALL.into_iter().zip(&bundle.channels) {
        match (owner, bundle.qualified[channel.index()]) {
            (Some(_), Some(attachment))
                if attachment.identity == bundle.identity && attachment.channel == channel => {}
            (None, None) => {}
            _ => return Err(LoanError::CoverageMismatch),
        }
    }
    Ok(())
}
/// Only this closed original law can establish the masked exclusion pairing.
pub(super) fn square(
    operation: &crate::CheckedOperation,
) -> Result<&Rc<KernelContract>, LoanError> {
    if operation.inputs().len() != 1 || operation.outputs().len() != 1 || operation.revision() != 1
    {
        return Err(LoanError::UnsupportedContract);
    }
    let NativeRecipe::Kernel(law) = &operation.recipe else {
        return Err(LoanError::UnsupportedContract);
    };
    if !matches!(law.recipe, Recipe::Elementwise(Elementwise::Square)) {
        return Err(LoanError::UnsupportedContract);
    }
    Ok(law)
}
impl Evidence {
    /// Authenticate original exclusion, output authority and revision without a live source lookup.
    pub(in crate::loan) fn validate(
        &self,
        destination: &Declaration,
        identity: Identity,
    ) -> Result<(), LoanError> {
        self.validate_revision(destination, identity, 1)
    }
    /// Ready admission has no numerical revision; evaluation installs the supported revision once.
    pub(super) fn validate_revision(
        &self,
        destination: &Declaration,
        identity: Identity,
        revision: u32,
    ) -> Result<(), LoanError> {
        let law = square(&self.operation)?;
        let exclusion = Rc::ptr_eq(&self.excluded_inputs, &self.operation.inputs)
            && Rc::ptr_eq(&self.excluded_outputs, &self.operation.outputs);
        let source_layout = self.demand.layout == self.source_identity.layout;
        let same_axis = destination.axis == self.source.axis;
        if !exclusion
            || !Rc::ptr_eq(law, &self.law)
            || self.destination != identity
            || self.revision != revision
            || !source_layout
            || !same_axis
        {
            return Err(LoanError::WrongLoan);
        }
        if destination.extent != self.source.extent
            || destination.coverage != self.source.coverage
            || self.outcomes.len() != destination.extent.rows()
        {
            return Err(LoanError::LayoutMismatch);
        }
        declarations(&self.source, &destination.symbols, &self.operation)?;
        for (index, attachment) in self.attachments.iter().enumerate() {
            let present = self.source_identity.channels[index].is_some();
            if present != attachment.is_some()
                || attachment.is_some_and(|attachment| {
                    attachment.identity != self.source_identity
                        || attachment.channel != HostChannel::ALL[index]
                })
            {
                return Err(LoanError::CoverageMismatch);
            }
        }
        Ok(())
    }
}
