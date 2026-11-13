// SPDX-License-Identifier: AGPL-3.0-only
//! Checked cell movement preserves complete original value and companion bundles.

use super::{ApplicationError, ApplicationRequest, PointwiseRequest, prepared::Request};
use crate::loan::{ExecutionAccount, LoanError, charge, registry::Bundle};
use crate::{EntryMap, Heap, KernelKind, MaskView};
use nrz_bindings::Budget;
use nrz_scalar::Scalar;

impl Heap {
    /// Retain a native original, gathered or product selection without an entry-sized map.
    /// Every retained map clone pins its layout record until dropped, including maps
    /// retained by checked operations; layout retirement then returns `PinnedUses`.
    /// # Errors
    /// Refuses foreign or stale masks, incompatible component extents and exhausted Work.
    pub fn cell_entry_map(
        &self,
        source: crate::Extent,
        demand: crate::MaskVersion,
        product: Option<(crate::CellProduct, crate::CellFactor)>,
        budget: &mut Budget,
    ) -> Result<EntryMap, LoanError> {
        budget.with_work_phase(super::FAMILY, "native copy descriptor", |budget| {
            charge(budget, 4)?;
            self.mask(demand, demand.layout())?;
            let layout = self
                .masks
                .layouts
                .get(demand.layout().key)?
                .owner()?
                .clone();
            EntryMap::selected_cells(source, layout, product).map_err(|_| LoanError::LayoutMismatch)
        })
    }

    /// Evaluate a unary law directly over native product or gather incidence in either format.
    pub fn request_projected_elementwise<P: Scalar + 'static>(
        &mut self,
        request: PointwiseRequest<P>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<P> {
        let origin = match super::chain::origin(&request.input) {
            Ok(origin) => origin,
            Err(error) => return ApplicationRequest::ApplicationRefused(error.into(), request),
        };
        let kind = match &request.operation.recipe {
            crate::binding::NativeRecipe::Kernel(law)
                if matches!(
                    law.recipe,
                    crate::kernels::contract::Recipe::ProjectedElementwise(..)
                ) =>
            {
                law.kind()
            }
            _ => {
                return ApplicationRequest::ApplicationRefused(
                    LoanError::UnsupportedContract.into(),
                    request,
                );
            }
        };
        super::admission::request_kind(self, request, kind, true, origin, account)
    }

    /// Copy selected original values and both companions into disjoint dense owners.
    ///
    /// The checked entry transform preserves component order within each copied cell.
    /// Repeated source cells remain dependent on the same original source. Gathered
    /// demand additionally authenticates its complete original-index correspondence.
    /// Refusal returns the unchanged request and source pin; output holes perform no loads.
    pub fn request_mapped_copy<P: Scalar + 'static>(
        &mut self,
        request: PointwiseRequest<P>,
        account: &mut ExecutionAccount,
    ) -> ApplicationRequest<P> {
        let origin = match super::chain::origin(&request.input) {
            Ok(origin) => origin,
            Err(error) => return ApplicationRequest::ApplicationRefused(error.into(), request),
        };
        super::admission::request_kind(
            self,
            request,
            KernelKind::TransformEntries,
            false,
            origin,
            account,
        )
    }
}

/// Original shape, descriptor, occupation and cell-map checks precede every output allocation.
pub(super) fn check<P: Scalar + 'static>(
    heap: &Heap,
    request: &Request<'_, P>,
    account: u64,
    origin: super::chain::Input,
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
    super::chain::authenticate(heap, request.input, origin, budget)?;
    let source = super::contract::source(request)?;
    let member = request.member()?;
    let map = member.map()?;
    let destination = request.destination;
    if map.source() != source.declaration.extent
        || map.target() != destination.extent
        || map.source().columns() != map.target().columns()
        || source.channels.iter().any(Option::is_none)
        || source.endpoints.is_some()
        || source.retained_law.is_some()
        || member.law().inputs.as_ref() != [map.source()]
        || member.law().output != map.target()
    {
        return Err(LoanError::UnsupportedContract.into());
    }
    if request.operation.inputs()[0].shape().extent() != map.source()
        || request.operation.outputs()[0].shape().extent() != map.target()
    {
        return Err(LoanError::LayoutMismatch.into());
    }
    exclude(request, budget)?;
    charge(budget, 4)?;
    if destination.coverage != source.declaration.coverage {
        return Err(LoanError::CoverageMismatch.into());
    }
    super::contract::declarations_with_shape(
        &source.declaration,
        &destination.symbols,
        request.operation.inputs()[0].descriptor(),
        request.operation.outputs()[0].descriptor(),
        false,
    )?;
    super::contract::attachments(source)?;
    let demand = heap
        .mask(request.demand, request.demand.layout())
        .map_err(LoanError::from)?;
    if demand.axis() != destination.axis {
        return Err(LoanError::LayoutMismatch.into());
    }
    correspondence(source, map, &demand, budget)?;
    let work =
        crate::kernels::contract::work(destination.components(), member.invocation_factor())?;
    let payload = destination
        .extent
        .entries()
        .checked_mul(3)
        .ok_or(LoanError::CountOverflow)?;
    request.limits.check(1, work.max(payload))?;
    Ok(())
}

/// Sparse demand cannot hide unsupported structural assertions in either original region.
fn exclude<P: Scalar>(
    request: &Request<'_, P>,
    budget: &mut Budget,
) -> Result<(), ApplicationError> {
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
    Ok(())
}

/// Every component follows the same source cell; a gathered branch cannot substitute its origin.
fn correspondence(
    source: &Bundle,
    map: &EntryMap,
    demand: &MaskView<'_>,
    budget: &mut Budget,
) -> Result<(), LoanError> {
    charge(budget, 3)?;
    let selected = map.selection();
    if selected.is_some_and(|layout| {
        layout.axis != demand.axis() || layout.gather.is_some() != demand.gather().is_some()
    }) {
        return Err(LoanError::LayoutMismatch);
    }
    let original_axis = demand
        .gather()
        .map_or_else(|| demand.axis(), crate::CellGather::original_axis);
    if let Some((product, factor)) = map.product() {
        if original_axis != product.axis()
            || source.declaration.axis != product.factors()[factor.index()]
            || (demand.gather().is_some() && selected.is_none())
        {
            return Err(LoanError::LayoutMismatch);
        }
    } else if original_axis != source.declaration.axis || selected.is_none() {
        return Err(LoanError::LayoutMismatch);
    }
    if let Some(gather) = demand.gather() {
        let stored = selected
            .and_then(|layout| layout.gather.as_ref())
            .ok_or(LoanError::LayoutMismatch)?;
        if !std::ptr::eq(gather, stored)
            || (map.product().is_none() && gather.source().layout() != source.identity.layout)
        {
            return Err(LoanError::LayoutMismatch);
        }
    }
    Ok(())
}

/// Checked first-component lookup determines a source cell after complete map validation.
pub(super) fn source_cell(map: &EntryMap, cell: usize) -> Result<usize, LoanError> {
    map.source_cell(cell).ok_or(LoanError::LayoutMismatch)
}

/// Prepaid per-cell incidence checks derive actual availability without reading numerical holes.
pub(super) fn filled(
    map: &EntryMap,
    source: &Bundle,
    demand: &MaskView<'_>,
    words: &mut [u64],
) -> Result<(), LoanError> {
    for cell in 0..map.target().rows() {
        let selected = demand.contains(cell)?;
        let Some(original) = map.source_cell(cell) else {
            if selected {
                return Err(LoanError::CoverageMismatch);
            }
            crate::loan::payload::set(words, cell, false);
            continue;
        };
        let available = crate::loan::payload::bit(&source.mask.words, original);
        crate::loan::payload::set(words, cell, selected && available);
    }
    Ok(())
}

/// Two native map steps, shared bundle coverage, demand and destination-bit store.
pub(super) fn availability_work(cells: usize, words: usize) -> Result<usize, LoanError> {
    cells
        .checked_mul(5)
        .and_then(|work| {
            words
                .checked_mul(4)
                .and_then(|fixed| work.checked_add(fixed))
        })
        .ok_or(LoanError::CountOverflow)
}
