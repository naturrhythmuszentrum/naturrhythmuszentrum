// SPDX-License-Identifier: AGPL-3.0-only
//! Total numerical entry coverage matched to bindings-owned exact symbol relations.

use crate::heap::identity::Key;
use crate::{Extent, Handle, Heap, HeapError, PublishedBundle, Value, ValueStamp};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits, QuantitySymbol};
use nrz_scalar::Scalar;

/// A checked declaration covering every numerical entry of its allocation.
#[derive(Clone, Debug)]
pub(crate) enum Sponsor {
    /// Closed homogeneous recipe, covering the complete declared rectangle.
    Homogeneous(QuantitySymbol),
    /// Per-entry parameter meanings and their exact mathematical targets.
    Parameter(nrz_bindings::ParameterSymbol),
    /// Full checked entry universe with any number of proved overlapping readers.
    Readings(std::rc::Rc<super::live::Group>),
}

/// One allocation sponsor and the current version of each covered entry.
#[derive(Clone, Debug)]
pub(crate) struct RegionCover {
    /// Actual descriptor occupation, independent of addresses.
    pub(crate) key: Key,
    /// Allocation sponsor revision, advanced by coarse replacement.
    pub(crate) stamp: ValueStamp,
    /// Complete semantic cover and the designated allocation sponsor.
    pub(crate) sponsor: Sponsor,
    /// Monotonic versions of the actual covered entries.
    pub(crate) versions: Vec<std::cell::Cell<u64>>,
    /// Additional readers admitted together with this allocation sponsor.
    pub(crate) readers: Vec<std::rc::Rc<super::live::Group>>,
}

/// One coverage inventory for homogeneous recipes and general physical readings.
#[derive(Debug, Default)]
pub(crate) struct Inventory {
    /// One full cover per occupied numerical allocation.
    entries: Vec<Option<RegionCover>>,
    /// Selected aliases retain the exact reading and all its structural dependencies.
    selections: Vec<Option<Selection>>,
    /// Exact no-storage declarations still participate in structural invalidation.
    observers: Vec<std::rc::Rc<super::live::Group>>,
}

/// An alias retains the symbols constructed only for its selected companion inventory.
#[derive(Debug)]
struct Selection {
    /// Exact descriptor occupation.
    key: Key,
    /// Original reading and complete live dependency closure.
    reading: super::live::selected::SelectedReading,
    /// Selected value and radius identities, prepared before alias publication.
    symbols: BindingSymbols,
}

impl Inventory {
    /// A selected reading belongs to this inventory and resolves through its allocation sponsor.
    pub(crate) fn selection(&self, key: Key) -> Option<&super::live::selected::SelectedReading> {
        self.selections
            .get(key.slot)
            .and_then(Option::as_ref)
            .filter(|selected| selected.key == key)
            .map(|selected| &selected.reading)
    }

    /// Resolve the exact selected companion inventory without rebuilding its declarations.
    pub(crate) fn selection_symbols(&self, key: Key) -> Option<&BindingSymbols> {
        self.selections
            .get(key.slot)
            .and_then(Option::as_ref)
            .filter(|selected| selected.key == key)
            .map(|selected| &selected.symbols)
    }

    /// Reserve immutable selected-alias metadata before publishing its descriptor.
    pub(crate) fn reserve_selection(&mut self, key: Key, limit: usize) -> Result<(), HeapError> {
        let end = key.slot.checked_add(1).ok_or(HeapError::Capacity)?;
        if end > limit {
            return Err(HeapError::Capacity);
        }
        if end > self.selections.len() {
            self.selections
                .try_reserve(end - self.selections.len())
                .map_err(|_| HeapError::Capacity)?;
            self.selections.resize_with(end, || None);
        }
        Ok(())
    }

    /// Install a prepared exact selection after its numerical alias is committed.
    pub(crate) fn publish_selection(
        &mut self,
        key: Key,
        selected: super::live::selected::SelectedReading,
        symbols: BindingSymbols,
    ) {
        if let Some(slot) = self.selections.get_mut(key.slot) {
            *slot = Some(Selection {
                key,
                reading: selected,
                symbols,
            });
        }
    }

    /// Resolve the allocation sponsor without inferring meaning from an address.
    pub(crate) fn cover(&self, key: Key) -> Result<&RegionCover, HeapError> {
        self.entries
            .get(key.slot)
            .and_then(Option::as_ref)
            .filter(|entry| entry.key == key)
            .ok_or(HeapError::MissingBinding)
    }

    /// Require the homogeneous recipe interpretation of the current region version.
    pub(crate) fn get(&self, key: Key, stamp: ValueStamp) -> Result<&QuantitySymbol, HeapError> {
        let entry = self.cover(key)?;
        match &entry.sponsor {
            Sponsor::Homogeneous(symbol) if entry.stamp == stamp => Ok(symbol),
            _ => Err(HeapError::MissingBinding),
        }
    }

    /// Reserve a complete homogeneous cover before any numerical allocation.
    pub(crate) fn prepare(
        &mut self,
        key: Key,
        stamp: ValueStamp,
        symbol: &QuantitySymbol,
        limit: usize,
    ) -> Result<RegionCover, HeapError> {
        let [rows, columns] = symbol.shape();
        let count = u64::from(rows) * u64::from(columns);
        self.reserve_cover(
            key,
            stamp,
            Sponsor::Homogeneous(symbol.clone()),
            usize::try_from(count).map_err(|_| HeapError::Capacity)?,
            limit,
        )
    }

    /// General readings have already checked total coverage and every actual intersection.
    pub(crate) fn prepare_readings(
        &mut self,
        key: Key,
        stamp: ValueStamp,
        group: &std::rc::Rc<super::live::Group>,
        count: usize,
        limit: usize,
    ) -> Result<RegionCover, HeapError> {
        self.reserve_cover(
            key,
            stamp,
            Sponsor::Readings(std::rc::Rc::clone(group)),
            count,
            limit,
        )
    }

    /// Prepare a complete independently declared parameter allocation.
    pub(crate) fn prepare_parameter(
        &mut self,
        key: Key,
        symbol: nrz_bindings::ParameterSymbol,
        limit: usize,
    ) -> Result<RegionCover, HeapError> {
        let count = symbol.entries().len();
        let stamp = ValueStamp {
            id: crate::ValueId(crate::heap::identity::identity()?),
            version: crate::ValueVersion(1),
        };
        self.reserve_cover(key, stamp, Sponsor::Parameter(symbol), count, limit)
    }

    /// Reserve the single sponsor occupation and all entry versions without publishing it.
    fn reserve_cover(
        &mut self,
        key: Key,
        stamp: ValueStamp,
        sponsor: Sponsor,
        count: usize,
        limit: usize,
    ) -> Result<RegionCover, HeapError> {
        if self.cover(key).is_ok_and(|entry| entry.stamp == stamp) {
            return Err(HeapError::IncompatibleOverlap);
        }
        let end = key.slot.checked_add(1).ok_or(HeapError::Capacity)?;
        if end > limit {
            return Err(HeapError::Capacity);
        }
        if end > self.entries.len() {
            self.entries
                .try_reserve(end - self.entries.len())
                .map_err(|_| HeapError::Capacity)?;
            self.entries.resize_with(end, || None);
        }
        let mut versions = super::buffer(count)?;
        versions.resize_with(count, || std::cell::Cell::new(1));
        Ok(RegionCover {
            key,
            stamp,
            sponsor,
            versions,
            readers: Vec::new(),
        })
    }

    /// Reserve an exact-only observer before publishing its no-storage declaration.
    pub(crate) fn reserve_observer(&mut self, limit: usize) -> Result<(), HeapError> {
        if self.observers.len() >= limit {
            return Err(HeapError::Capacity);
        }
        self.observers
            .try_reserve(1)
            .map_err(|_| HeapError::Capacity)
    }

    /// Exact-only declarations have no numerical sponsor slot but retain all proof obligations.
    pub(crate) fn publish_observer(&mut self, group: std::rc::Rc<super::live::Group>) {
        self.observers.push(group);
    }

    /// All groups are visited once when a write can invalidate their retained evidence.
    pub(crate) fn groups(&self) -> Vec<std::rc::Rc<super::live::Group>> {
        let mut groups = self.observers.clone();
        for cover in self.entries.iter().flatten() {
            if let Sponsor::Readings(group) = &cover.sponsor
                && !groups.iter().any(|old| std::rc::Rc::ptr_eq(old, group))
            {
                groups.push(std::rc::Rc::clone(group));
            }
            for group in &cover.readers {
                if !groups.iter().any(|old| std::rc::Rc::ptr_eq(old, group)) {
                    groups.push(std::rc::Rc::clone(group));
                }
            }
        }
        groups
    }

    /// Install already checked covers into already reserved metadata slots.
    pub(crate) fn publish(&mut self, prepared: Vec<RegionCover>) {
        for entry in prepared {
            if let Some(slot) = self.entries.get_mut(entry.key.slot) {
                *slot = Some(entry);
            }
        }
    }
}

/// Preserve explicit exact-construction ceilings separately from numerical work.
pub(crate) fn budget() -> Result<Budget, HeapError> {
    Budget::new(Limits::default()).map_err(|cause| nrz_bindings::AdmissionError::from(cause).into())
}

/// Translate finite numerical extents into literal CAS sort extents.
pub(crate) fn shape(extent: Extent) -> Result<[u32; 2], HeapError> {
    Ok([
        u32::try_from(extent.rows()).map_err(|_| HeapError::ExtentOverflow)?,
        u32::try_from(extent.columns()).map_err(|_| HeapError::ExtentOverflow)?,
    ])
}

/// Prepare declarations before any numerical region is allocated.
pub(crate) fn declare(
    quantity: &Descriptor,
    extent: Extent,
    companions: [bool; 2],
    ledger: &mut Budget,
) -> Result<BindingSymbols, HeapError> {
    Ok(BindingSymbols::declare(
        quantity,
        shape(extent)?,
        companions,
        ledger,
    )?)
}

/// Resolve the entire future descriptor bundle before any floating slab is allocated.
pub(crate) fn planned_bundle<P: Scalar>(
    scope: u64,
    keys: &[Key],
    plan: &crate::heap::publication::SchemaPlan,
) -> Result<PublishedBundle<P>, HeapError> {
    let expected = 1
        + usize::from(plan.bound.is_some())
        + usize::from(plan.evaluation.is_some())
        + 3 * usize::from(plan.contributions.is_some());
    if keys.len() != expected {
        return Err(HeapError::LengthMismatch);
    }
    let get = |index| keys.get(index).copied().ok_or(HeapError::Capacity);
    let bound = plan
        .bound
        .map(|_| get(1).map(|key| Handle::new(scope, key)))
        .transpose()?;
    let evaluation = plan
        .evaluation
        .map(|_| get(2).map(|key| Handle::new(scope, key)))
        .transpose()?;
    let contributions = if plan.contributions.is_some() {
        Some([
            Handle::new(scope, get(3)?),
            Handle::new(scope, get(4)?),
            Handle::new(scope, get(5)?),
        ])
    } else {
        None
    };
    Ok(PublishedBundle {
        value: Handle::new(scope, get(0)?),
        bound,
        evaluation,
        contributions,
    })
}

/// Match every value and companion declaration and reserve complete covers before allocation.
pub(crate) fn prepare<P: Scalar>(
    heap: &mut Heap,
    symbols: &BindingSymbols,
    bundle: PublishedBundle<P>,
    stamp: ValueStamp,
) -> Result<Vec<RegionCover>, HeapError> {
    let _binding = nrz_bindings::SymbolBinding::check(
        symbols.clone(),
        [
            Some(bundle.value().key),
            bundle.bound().map(|handle| handle.key),
            bundle.evaluation().map(|handle| handle.key),
        ]
        .into_iter()
        .flatten()
        .collect(),
    )?;
    let bounds = symbols.bound().is_some() == bundle.bound().is_some();
    let errors = symbols.error().is_some() == bundle.evaluation().is_some();
    if !bounds || !errors {
        return Err(nrz_bindings::AdmissionError::CompanionMismatch.into());
    }
    let mut covers = super::buffer(3)?;
    covers.push(heap.symbols.prepare(
        bundle.value().key,
        stamp,
        symbols.value(),
        heap.limits.descriptors,
    )?);
    for (handle, symbol) in [
        (bundle.bound(), symbols.bound()),
        (bundle.evaluation(), symbols.error()),
    ] {
        if let (Some(handle), Some(symbol)) = (handle, symbol) {
            if handle.key == bundle.value().key
                || covers.iter().any(|owner| owner.key == handle.key)
            {
                return Err(HeapError::IncompatibleOverlap);
            }
            covers.push(heap.symbols.prepare(
                handle.key,
                stamp,
                symbol,
                heap.limits.descriptors,
            )?);
        }
    }
    if let Some(contributions) = bundle.contributions {
        for handle in contributions {
            let declaration = super::parameters::radii(symbols.value())?;
            covers.push(heap.symbols.prepare_parameter(
                handle.key,
                declaration,
                heap.limits.descriptors,
            )?);
        }
    }
    Ok(covers)
}

/// Resolve actual numerical ownership and target agreement before semantic checking.
pub(crate) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
) -> Result<BindingSymbols, HeapError> {
    if let Some(selected) = heap.symbols.selection(handle.key) {
        selected.validate(heap)?;
        return heap
            .symbols
            .selection_symbols(handle.key)
            .cloned()
            .ok_or(HeapError::MissingBinding);
    }
    let base = heap.base_handle(handle)?;
    let view = heap.value(base)?;
    let stamp = view.stamp();
    let value = heap.symbols.get(base.key, stamp)?.clone();
    value.check_descriptor(heap.binding_quantity(base)?)?;
    if value.shape() != shape(view.region().extent())? {
        return Err(HeapError::LengthMismatch);
    }
    let mut companions = Vec::new();
    for locator in [view.bound(), view.evaluation()] {
        let symbol = locator
            .map(|locator| {
                let companion = heap.companion(locator)?;
                let symbol = heap.symbols.get(locator.key, stamp)?.clone();
                symbol.check_descriptor(heap.binding_quantity(locator)?)?;
                if companion.target() != base || companion.target_stamp() != stamp {
                    return Err(HeapError::StaleEvidence);
                }
                Ok::<_, HeapError>(symbol)
            })
            .transpose()?;
        companions.push(symbol);
    }
    let mut companions = companions.into_iter();
    Ok(BindingSymbols::check(
        value,
        companions.next().flatten(),
        companions.next().flatten(),
    )?)
}

/// Recheck the exact planned symbol inventory against every reserved numerical target.
pub(crate) fn validate<P: Scalar>(
    covers: &[RegionCover],
    symbols: &[BindingSymbols],
    bundles: &[PublishedBundle<P>],
    outputs: &[super::OutputReservation],
) -> Result<(), HeapError> {
    if symbols.len() != bundles.len() || symbols.len() != outputs.len() {
        return Err(HeapError::LengthMismatch);
    }
    let mut covers = covers
        .iter()
        .filter(|cover| matches!(cover.sponsor, Sponsor::Homogeneous(_)));
    for ((symbols, bundle), output) in symbols.iter().zip(bundles).zip(outputs) {
        let mut expected = vec![(bundle.value().key, symbols.value())];
        for (handle, symbol) in [
            (bundle.bound(), symbols.bound()),
            (bundle.evaluation(), symbols.error()),
        ] {
            match (handle, symbol) {
                (Some(handle), Some(symbol)) => expected.push((handle.key, symbol)),
                (None, None) => {}
                _ => return Err(nrz_bindings::AdmissionError::CompanionMismatch.into()),
            }
        }
        for (key, symbol) in expected {
            let owner = covers.next().ok_or(HeapError::MissingBinding)?;
            if owner.key != key
                || owner.stamp != output.stamp()
                || !matches!(&owner.sponsor, Sponsor::Homogeneous(actual) if actual == symbol)
            {
                return Err(HeapError::MissingBinding);
            }
        }
    }
    if covers.next().is_some() {
        return Err(HeapError::IncompatibleOverlap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_testing::{TestResult, operation};
    use crate::{Binding, ExecutionLimits, HeapLimits, OperationError};

    /// A live value cannot enter copy without its complete current binding cover.
    /// ´test:unit:copy-refuses-missing-binding-cover´
    #[test]
    fn copy_refuses_missing_binding_cover() -> TestResult {
        for companion in [false, true] {
            let mut heap = Heap::new(HeapLimits::default())?;
            let source = heap.import_declared_bounds(
                Extent::new(1, 1)?,
                &Descriptor::ratio(),
                &[2.0_f64],
                &[0.5],
            )?;
            let key = if companion {
                source.bound().ok_or("bound")?.key
            } else {
                source.value().key
            };
            heap.symbols.entries[key.slot] = None;
            assert!(matches!(
                Binding::new(
                    operation(1, 1)?,
                    vec![source.value()],
                    ExecutionLimits::default()
                )
                .validate(&mut heap),
                Err(OperationError::Heap(HeapError::MissingBinding))
            ));
            assert_eq!(heap.descriptors.iter().count(), 2);
        }
        Ok(())
    }

    /// Equal units and bytes cannot authorize an unrelated overlapping declaration.
    /// ´test:unit:region-version-refuses-unrelated-overlap´
    #[test]
    fn region_version_refuses_unrelated_overlap() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let stamp = heap.value(source.value())?.stamp();
        let other = declare(
            &Descriptor::ratio(),
            Extent::new(1, 1)?,
            [false, false],
            &mut budget()?,
        )?;
        assert!(matches!(
            heap.symbols.prepare(
                source.value().key,
                stamp,
                other.value(),
                heap.limits.descriptors
            ),
            Err(HeapError::IncompatibleOverlap)
        ));
        assert_eq!(heap.value(source.value())?.region().get(0, 0)?, 2.0);
        Ok(())
    }

    /// A foreign companion symbol is refused even though the live numeric target agrees.
    /// ´test:unit:copy-refuses-wrong-companion-symbol´
    #[test]
    fn copy_refuses_wrong_companion_symbol() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source = heap.import_declared_bounds(
            Extent::new(1, 1)?,
            &Descriptor::ratio(),
            &[2.0_f64],
            &[0.5],
        )?;
        let other = declare(
            &Descriptor::ratio(),
            Extent::new(1, 1)?,
            [true, false],
            &mut budget()?,
        )?;
        let key = source.bound().ok_or("bound")?.key;
        heap.symbols.entries[key.slot]
            .as_mut()
            .ok_or("owner")?
            .sponsor = Sponsor::Homogeneous(other.bound().ok_or("symbol")?.clone());
        assert!(matches!(
            Binding::new(
                operation(1, 1)?,
                vec![source.value()],
                ExecutionLimits::default()
            )
            .validate(&mut heap),
            Err(OperationError::Heap(HeapError::Symbols(
                nrz_bindings::AdmissionError::CompanionMismatch
            )))
        ));
        Ok(())
    }

    /// Publication refuses a substituted planned result without exposing any output prefix.
    /// ´test:unit:copy-result-symbol-substitution-is-atomic´
    #[test]
    fn copy_result_symbol_substitution_is_atomic() -> TestResult {
        let mut heap = Heap::new(HeapLimits::default())?;
        let source =
            heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[2.0_f64])?;
        let recipe =
            crate::OperationDraft::copy(vec![operation(1, 1)?.inputs()[0].shape().clone(); 2])
                .check(ExecutionLimits::default())?;
        let mut bound = Binding::new(recipe, vec![source.value(); 2], ExecutionLimits::default())
            .validate(&mut heap)?;
        let candidate = bound.evaluate(bound.normalized_inputs()?)?;
        bound.reservations.symbols[1] = declare(
            &Descriptor::ratio(),
            Extent::new(1, 1)?,
            [false, false],
            &mut budget()?,
        )?;
        assert!(bound.publish(candidate).is_err());
        assert_eq!(heap.descriptors.iter().count(), 1);
        assert_eq!(heap.provenance_usage().records(), 0);
        Ok(())
    }
}
