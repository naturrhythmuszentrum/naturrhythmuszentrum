// SPDX-License-Identifier: AGPL-3.0-only
//! Numerical owner inventory matched to bindings-owned exact symbol relations.

use crate::heap::identity::Key;
use crate::{Extent, Handle, Heap, HeapError, PublishedBundle, Value, ValueStamp};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits, QuantitySymbol};
use nrz_scalar::Scalar;

/// One region version and its unique opaque symbol owner.
#[derive(Clone, Debug)]
pub(crate) struct Owner {
    /// Exact descriptor occupation.
    key: Key,
    /// Value version, or exact target version for a companion.
    stamp: ValueStamp,
    /// Immutable bindings declaration.
    symbol: QuantitySymbol,
}

/// Slot-indexed inventory; aliases resolve through their existing base owner.
#[derive(Debug, Default)]
pub(crate) struct Owners {
    /// Empty entries reserve metadata capacity without exposing planned values.
    entries: Vec<Option<Owner>>,
}

impl Owners {
    /// Require the one owner of this exact current region version.
    pub(crate) fn get(&self, key: Key, stamp: ValueStamp) -> Result<&QuantitySymbol, HeapError> {
        self.entries
            .get(key.slot)
            .and_then(Option::as_ref)
            .filter(|entry| entry.key == key && entry.stamp == stamp)
            .map(|entry| &entry.symbol)
            .ok_or(HeapError::MissingSymbolOwner)
    }

    /// Reserve a slot before publication, refusing a second owner of the same version.
    pub(crate) fn prepare(
        &mut self,
        key: Key,
        stamp: ValueStamp,
        symbol: &QuantitySymbol,
        limit: usize,
    ) -> Result<Owner, HeapError> {
        if self.get(key, stamp).is_ok() {
            return Err(HeapError::DuplicateSymbolOwner);
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
        Ok(Owner {
            key,
            stamp,
            symbol: symbol.clone(),
        })
    }

    /// Install only already checked entries into already reserved metadata slots.
    pub(crate) fn publish(&mut self, prepared: Vec<Owner>) {
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

/// Match the fixed value/companion inventory and reserve every owner before commit.
pub(crate) fn prepare<P: Scalar>(
    heap: &mut Heap,
    symbols: &BindingSymbols,
    bundle: PublishedBundle<P>,
    stamp: ValueStamp,
) -> Result<Vec<Owner>, HeapError> {
    let _binding = nrz_bindings::SymbolBinding::check(
        symbols.clone(),
        [
            Some(bundle.value().key),
            bundle.bound().map(|handle| handle.key),
            bundle.evaluation().map(|handle| handle.key),
        ],
    )?;
    let bounds = symbols.bound().is_some() == bundle.bound().is_some();
    let errors = symbols.error().is_some() == bundle.evaluation().is_some();
    if !bounds || !errors {
        return Err(nrz_bindings::AdmissionError::CompanionMismatch.into());
    }
    let mut owners = super::buffer(3)?;
    owners.push(heap.symbols.prepare(
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
                || owners.iter().any(|owner| owner.key == handle.key)
            {
                return Err(HeapError::DuplicateSymbolOwner);
            }
            owners.push(heap.symbols.prepare(
                handle.key,
                stamp,
                symbol,
                heap.limits.descriptors,
            )?);
        }
    }
    Ok(owners)
}

/// Resolve actual numerical ownership and target agreement before semantic checking.
pub(crate) fn capture<P: Scalar + 'static>(
    heap: &Heap,
    handle: Handle<Value, P>,
) -> Result<BindingSymbols, HeapError> {
    let base = heap.base_handle(handle)?;
    let view = heap.value(base)?;
    let stamp = view.stamp();
    let value = heap.symbols.get(base.key, stamp)?.clone();
    value.check_descriptor(view.region().descriptor())?;
    if value.shape() != shape(view.region().extent())? {
        return Err(HeapError::LengthMismatch);
    }
    let mut companions = Vec::new();
    for locator in [view.bound(), view.evaluation()] {
        let symbol = locator
            .map(|locator| {
                let companion = heap.companion(locator)?;
                let symbol = heap.symbols.get(locator.key, stamp)?.clone();
                symbol.check_descriptor(companion.region().descriptor())?;
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
    owners: &[Owner],
    symbols: &[BindingSymbols],
    bundles: &[PublishedBundle<P>],
    outputs: &[super::OutputReservation],
) -> Result<(), HeapError> {
    if symbols.len() != bundles.len() || symbols.len() != outputs.len() {
        return Err(HeapError::LengthMismatch);
    }
    let mut owners = owners.iter();
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
            let owner = owners.next().ok_or(HeapError::MissingSymbolOwner)?;
            if owner.key != key || owner.stamp != output.stamp() || &owner.symbol != symbol {
                return Err(HeapError::MissingSymbolOwner);
            }
        }
    }
    if owners.next().is_some() {
        return Err(HeapError::DuplicateSymbolOwner);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding_testing::{TestResult, operation};
    use crate::{Binding, ExecutionLimits, HeapLimits, OperationError};

    /// A live value cannot enter copy without exactly one current symbol owner.
    /// ´test:unit:copy-refuses-missing-symbol-owner´
    #[test]
    fn copy_refuses_missing_symbol_owner() -> TestResult {
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
                Err(OperationError::Heap(HeapError::MissingSymbolOwner))
            ));
            assert_eq!(heap.descriptors.iter().count(), 2);
        }
        Ok(())
    }

    /// Equal units and bytes cannot authorize another owner of an existing version.
    /// ´test:unit:region-version-refuses-double-symbol-owner´
    #[test]
    fn region_version_refuses_double_symbol_owner() -> TestResult {
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
            Err(HeapError::DuplicateSymbolOwner)
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
            .symbol = other.bound().ok_or("symbol")?.clone();
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
