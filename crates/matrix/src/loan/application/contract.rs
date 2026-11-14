// SPDX-License-Identifier: AGPL-3.0-only
//! Original meanings and full-shape exclusion precede every numerical allocation.

use super::{ApplicationError, Evidence, prepared::Request};
use crate::Heap;
use crate::loan::{
    HostChannel, LoanError, charge,
    registry::{Bundle, Declaration, Identity},
};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;
use std::rc::Rc;

/// Borrow the exact source owner without creating another numerical edge.
pub(super) fn source<'a, P: Scalar>(request: &Request<'a, P>) -> Result<&'a Bundle, LoanError> {
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
    request: &Request<'_, P>,
    account: u64,
    origin: super::chain::Input,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
    if request.binary.is_some() {
        return super::binary::check_request(heap, request, account, budget);
    }
    let projected = matches!(&request.operation.recipe, crate::binding::NativeRecipe::Kernel(law)
        if matches!(law.recipe, crate::kernels::contract::Recipe::ProjectedElementwise(..)));
    if projected != request.projected_entry {
        return Err(LoanError::UnsupportedContract.into());
    }
    if request.expected == crate::KernelKind::TransformEntries || projected {
        return super::movement::check(heap, request, account, origin, budget);
    }
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
    super::chain::authenticate(heap, request.input, origin, budget)?;
    let bundle = source(request)?;
    let member = request.member()?;
    let law = member.law();
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
        request.operation.inputs()[0].descriptor(),
        request.operation.outputs()[0].descriptor(),
    )?;
    member.representation::<P>(
        &bundle.declaration,
        &destination.symbols,
        destination.axis,
        destination.coverage,
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
    let work = crate::kernels::contract::work(extent.columns(), member.invocation_factor())?;
    let payload = extent
        .entries()
        .checked_mul(member.roles())
        .ok_or(LoanError::CountOverflow)?;
    request.limits.check(1, work.max(payload))?;
    Ok(())
}
/// Match the independently declared result convention, including exact fresh channel identities.
pub(super) fn declarations(
    source: &Declaration,
    output: &nrz_bindings::BindingSymbols,
    input: &nrz_bindings::Descriptor,
    result: &nrz_bindings::Descriptor,
) -> Result<(), LoanError> {
    declarations_with_shape(source, output, input, result, true)
}
/// Movement preserves descriptors and original targets while its checked map changes shape.
pub(super) fn declarations_with_shape(
    source: &Declaration,
    output: &nrz_bindings::BindingSymbols,
    input: &nrz_bindings::Descriptor,
    result: &nrz_bindings::Descriptor,
    same_shape: bool,
) -> Result<(), LoanError> {
    source
        .symbols
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    output
        .check_import()
        .map_err(|_| LoanError::StaleDependency)?;
    if source.symbols.value().descriptor() != input || output.value().descriptor() != result {
        return Err(LoanError::UnsupportedContract);
    }
    let shape_mismatch = same_shape && output.value().shape() != source.symbols.value().shape();
    if shape_mismatch || output.bound().is_none() || output.error().is_none() {
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
pub(super) fn attachments(bundle: &Bundle) -> Result<(), LoanError> {
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
        let member =
            super::member::Member::check(&self.operation, self.law.kind(), self.prepared.as_ref())?;
        if member.is_binary() != self.endpoint_tokens.binary().is_some() {
            return Err(LoanError::WrongLoan);
        }
        if let Some(side) = self.endpoint_tokens.binary() {
            side.validate(self, destination)?;
        }

        if let Some(prepared) = &self.prepared {
            let original = Rc::ptr_eq(&prepared.operation.inputs, &self.operation.inputs)
                && Rc::ptr_eq(&prepared.operation.outputs, &self.operation.outputs);
            if !original {
                return Err(LoanError::WrongLoan);
            }
            prepared.validate(&self.source, destination)?;
        }
        // Copies inherit their source scope, authenticated by the capture seal in chain.
        let arithmetic = !member.is_transform();
        let wrong_scope = self.error_scope != member.error_scope(self.error_scope);
        if arithmetic && wrong_scope {
            return Err(LoanError::WrongLoan);
        }
        self.validate_mapping(member, destination, identity, revision)?;
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
    /// Constant-size immutable map and target seals survive retirement of native demand storage.
    fn validate_mapping(
        &self,
        member: super::member::Member<'_>,
        destination: &Declaration,
        identity: Identity,
        revision: u32,
    ) -> Result<(), LoanError> {
        let law = member.law();
        let exclusion = Rc::ptr_eq(&self.excluded_inputs, &self.operation.inputs)
            && Rc::ptr_eq(&self.excluded_outputs, &self.operation.outputs);
        let (source_layout, same_axis) = if member.is_projected() {
            let (layout, axis) = self
                .endpoint_tokens
                .movement()
                .ok_or(LoanError::WrongLoan)?;
            (self.demand.layout == layout, destination.axis == axis)
        } else if let Some(side) = self.endpoint_tokens.binary() {
            let axis = if self.source.extent == law.output {
                self.source.axis
            } else {
                side.source.axis
            };
            (
                self.demand.layout == side.original.demand_layout(),
                destination.axis == axis,
            )
        } else {
            (
                self.demand.layout == self.source_identity.layout,
                destination.axis == self.source.axis,
            )
        };
        if member.is_projected() != self.endpoint_tokens.movement().is_some() {
            return Err(LoanError::WrongLoan);
        }
        if !exclusion
            || !Rc::ptr_eq(law, &self.law)
            || self.destination != identity
            || self.revision != revision
            || !source_layout
            || !same_axis
        {
            return Err(LoanError::WrongLoan);
        }
        let expected_extent = if member.is_projected() {
            member.map()?.target()
        } else if member.is_binary() {
            law.output
        } else {
            self.source.extent
        };
        if destination.extent != expected_extent
            || destination.coverage != self.source.coverage
            || self.outcomes.len() != destination.extent.rows()
        {
            return Err(LoanError::LayoutMismatch);
        }
        declarations_with_shape(
            &self.source,
            &destination.symbols,
            self.operation.inputs()[0].descriptor(),
            self.operation.outputs()[0].descriptor(),
            !member.is_projected() && !member.is_binary(),
        )?;
        if member.is_projected() && member.map()?.source() != self.source.extent {
            return Err(LoanError::WrongLoan);
        }
        member.representation_format(
            identity.format,
            &self.source,
            &destination.symbols,
            destination.axis,
            destination.coverage,
        )?;
        Ok(())
    }
}
