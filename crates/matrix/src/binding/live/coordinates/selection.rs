// SPDX-License-Identifier: AGPL-3.0-only
//! Checked restored projections retain the original immutable numerical and exact evidence.

use super::{CoordinateContract, CoordinateError, CoordinateReading};
use crate::binding::live::{
    Group, ReadingBinding, RegionTicket, ResultDefinition, selected::SelectedReading,
};
use crate::{
    CompanionAdapter, Coverage, Heap, HeapError, ReadingAdapter, ReadingCoverage, ValueBinding,
};
use nrz_bindings::{
    BindingSymbols, Budget,
    readings::{
        PhysicalReading,
        algebra::{Cas, Location, limits::BudgetKind},
    },
};
use nrz_scalar::{ArithmeticContext, Scalar};
use std::{any::Any, fmt::Debug, rc::Rc};

/// One retained snapshot can be shared by native inputs of either supported precision.
pub(crate) trait RestoredSource: Debug {
    /// Original selected physical definition.
    fn reading(&self) -> &PhysicalReading;
    /// Original companions retain both ordered covariance targets.
    fn companions(&self) -> &[CompanionAdapter<usize>];
    /// Fixed normalization and restoration references.
    fn contract(&self) -> &CoordinateContract;
    /// Original assurance, independent of generated radius presence.
    fn coverage(&self) -> Coverage;
    /// Safe recovery of the sealed original precision.
    fn as_any(&self) -> &dyn Any;
}

impl<P: Scalar + 'static> RestoredSource for CoordinateReading<P> {
    fn reading(&self) -> &PhysicalReading {
        self.source().reading()
    }
    fn companions(&self) -> &[CompanionAdapter<usize>] {
        self.source().companions()
    }
    fn contract(&self) -> &CoordinateContract {
        self.contract()
    }
    fn coverage(&self) -> Coverage {
        self.coverage()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A sized owner keeps type erasure out of the shared result-definition enum.
#[derive(Debug)]
pub(crate) struct Origin {
    /// Only coordinate selections pay for the dynamic precision-specific snapshot.
    pub(crate) snapshot: Box<dyn RestoredSource>,
}

/// Exact declarations and their full live source closure precede numerical copying.
#[derive(Debug)]
pub(crate) struct Plan {
    /// Immutable original snapshot, including both coordinate images.
    pub(crate) origin: Rc<Origin>,
    /// Fresh derived value and both radius associations.
    pub(crate) symbols: BindingSymbols,
    /// Original component representation mapped onto its own contiguous result.
    coverage: ReadingCoverage<usize>,
    /// Original immutable graph authority.
    graph: Rc<Cas>,
    /// Original state and its complete flattened producer closure.
    sources: Vec<SelectedReading>,
}

impl Heap {
    /// Publish a restored homogeneous component as an ordinary native value binding.
    ///
    /// The selected reading must be a proved view in the original source group. Restored
    /// centers, totals and numerical errors are copied together; no supplied float enters
    /// this operation. The original snapshot, companions and fixed references remain live
    /// dependencies, and generated radii never upgrade the original assurance.
    /// # Errors
    /// Refuses normalized or stale snapshots, changed precision/arithmetic, unrelated views,
    /// mixed or point selections and cumulative resource or allocation failures atomically.
    pub fn select_restored_reading<P: Scalar + 'static>(
        &mut self,
        restored: &CoordinateReading<P>,
        selected: &ReadingBinding<P>,
        context: &ArithmeticContext,
        budget: &mut Budget,
    ) -> Result<ValueBinding<P>, CoordinateError> {
        self.validate_reading(restored.source())?;
        self.validate_reading(selected)?;
        if restored.direction() != nrz_bindings::readings::CoordinateDirection::Restore
            || restored.0.revision != context.revision()
            || !Rc::ptr_eq(&restored.source().group, &selected.group)
        {
            return Err(CoordinateError::Identity);
        }
        let bytes = reservation(restored, selected, budget)?;
        budget.with_retained_payload(bytes, AT, |budget| {
            let positions = positions(restored.source(), selected, budget)?;
            let plan = plan(restored, selected, &positions, budget)?;
            let channels = channels(restored, &positions, budget)?;
            self.publish_coordinate_selection(plan, &channels, budget)
        })
    }

    /// Inspect the exact original restoration retained by a live native selection.
    /// # Errors
    /// Refuses unrelated, foreign, stale or differently typed bindings.
    pub fn coordinate_origin<P: Scalar + 'static>(
        &self,
        binding: &ValueBinding<P>,
    ) -> Result<&CoordinateReading<P>, CoordinateError> {
        self.value_by_symbol(binding.symbol(), binding)?;
        self.symbols
            .selection(binding.handle().key)
            .and_then(SelectedReading::restored)
            .and_then(|source| source.as_any().downcast_ref::<CoordinateReading<P>>())
            .ok_or(CoordinateError::Identity)
    }
}

/// One finite caller-ledger owner covers projection and publication metadata.
pub(crate) const AT: Location = Location::new("restored coordinate selection");

/// The existing witness envelope bounds cloned exact metadata; numerical copies have a separate share.
fn reservation<P: Scalar>(
    restored: &CoordinateReading<P>,
    selected: &ReadingBinding<P>,
    budget: &mut Budget,
) -> Result<u64, CoordinateError> {
    let entries = selected.adapter().entries().len();
    let witnesses = u64::try_from(
        selected
            .adapter()
            .representation()
            .witness()
            .exported_byte_len(),
    )
    .map_err(|_| HeapError::ExtentOverflow)?;
    let ancestors = restored
        .source()
        .group
        .antecedents
        .len()
        .checked_add(1)
        .ok_or(HeapError::ExtentOverflow)?;
    let metadata = ancestors
        .checked_mul(std::mem::size_of::<SelectedReading>())
        .ok_or(HeapError::ExtentOverflow)?;
    let bytes = witnesses
        .checked_mul(8)
        .and_then(|value| value.checked_add(restored.0.bytes))
        .and_then(|value| value.checked_add(4096))
        .and_then(|value| value.checked_add(u64::try_from(entries).ok()?.checked_mul(512)?))
        .and_then(|value| value.checked_add(u64::try_from(metadata).ok()?))
        .ok_or(HeapError::ExtentOverflow)?;
    budget.work(
        u64::try_from(
            entries
                .checked_add(ancestors)
                .ok_or(HeapError::ExtentOverflow)?,
        )
        .map_err(|_| HeapError::ExtentOverflow)?,
        AT,
    )?;
    budget.check(BudgetKind::StagedPayloadBytes, bytes, AT)?;
    budget.check(BudgetKind::OutputBytes, bytes, AT)?;
    Ok(bytes)
}

/// Actual storage-copy positions must agree with the retained exact family projection.
fn positions<P: Scalar>(
    source: &ReadingBinding<P>,
    selected: &ReadingBinding<P>,
    budget: &mut Budget,
) -> Result<Vec<usize>, CoordinateError> {
    let mut result = crate::binding::buffer(selected.adapter().entries().len())?;
    for entry in selected.adapter().entries() {
        budget.work(
            u64::try_from(source.adapter().entries().len())
                .map_err(|_| HeapError::ExtentOverflow)?,
            AT,
        )?;
        result.push(
            source
                .adapter()
                .entries()
                .iter()
                .position(|candidate| candidate == entry)
                .ok_or(CoordinateError::Identity)?,
        );
    }
    Ok(result)
}

/// Bindings checks exact projection and derived symbols before any output allocation.
fn plan<P: Scalar + 'static>(
    restored: &CoordinateReading<P>,
    selected: &ReadingBinding<P>,
    positions: &[usize],
    budget: &mut Budget,
) -> Result<Plan, CoordinateError> {
    let graph = Rc::clone(&selected.group.graph);
    let symbols = BindingSymbols::restored_reading(
        Rc::clone(&graph),
        restored.source().reading(),
        selected.reading(),
        positions,
        budget,
    )
    .map_err(HeapError::from)?;
    let entries = (0..positions.len())
        .map(|index| {
            Ok(crate::EntryTicket {
                region: 0,
                index: u64::try_from(index).map_err(|_| HeapError::ExtentOverflow)?,
            })
        })
        .collect::<Result<Vec<_>, HeapError>>()?;
    let adapter = ReadingAdapter::new(
        selected.adapter().representation().clone(),
        entries.clone(),
        budget,
    )
    .map_err(crate::LiveBindingError::from)?;
    let coverage = ReadingCoverage::new(entries, vec![adapter], Vec::new(), budget)
        .map_err(crate::LiveBindingError::from)?;
    let mut sources = restored.source().group.antecedents.clone();
    sources.push(restored.source().selected());
    Ok(Plan {
        origin: Rc::new(Origin {
            snapshot: Box::new(restored.clone()),
        }),
        symbols,
        coverage,
        graph,
        sources,
    })
}

/// A bounded scatter selects the complete restored channels with no floating arithmetic.
fn channels<P: Scalar>(
    restored: &CoordinateReading<P>,
    positions: &[usize],
    budget: &mut Budget,
) -> Result<[Vec<P>; 3], CoordinateError> {
    super::numeric::reserve::<P>(positions.len(), 6, budget)?;
    budget.work(
        u64::try_from(positions.len())
            .map_err(|_| HeapError::ExtentOverflow)?
            .checked_mul(3)
            .ok_or(HeapError::ExtentOverflow)?,
        AT,
    )?;
    let [centers, totals, errors] =
        [restored.centers(), restored.totals(), restored.errors()].map(|values| {
            positions
                .iter()
                .map(|&index| values.get(index).copied().ok_or(HeapError::InvalidLayout))
                .collect::<Result<Vec<_>, _>>()
        });
    Ok([centers?, totals?, errors?])
}

/// Native tickets preserve the selected source closure without replaying its original proofs.
pub(crate) fn group(
    plan: Plan,
    tickets: Vec<RegionTicket>,
    native: Vec<crate::binding::live::companions::NativeCompanion>,
    scope: u64,
    precision: nrz_scalar::Precision,
    budget: &mut Budget,
) -> Result<Rc<Group>, CoordinateError> {
    let mut group = crate::binding::live::dependencies::prepare(
        plan.graph,
        plan.coverage,
        Vec::new(),
        tickets,
        crate::DependencyPolicy::WholeBase,
        (scope, precision),
        budget,
    )?;
    let data = Rc::get_mut(&mut group).ok_or(HeapError::Capacity)?;
    data.homogeneous = vec![Some(plan.symbols)];
    data.reads = crate::binding::live::dependencies::ReadDependencies::Operands;
    data.antecedents = plan.sources;
    data.native = native;
    data.result_definition = Some(ResultDefinition::Coordinate(plan.origin));
    Ok(group)
}
