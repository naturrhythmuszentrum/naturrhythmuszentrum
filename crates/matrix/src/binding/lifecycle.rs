// SPDX-License-Identifier: AGPL-3.0-only
//! Symbol-selected values and explicit pending/publication state for planned results.
//!
//! # Symbol-selected import and result lifecycle
//!
//! Imports prepare symbols before allocating numerical storage. Copy preparation derives fresh result and companion symbols before allocating its private slabs, and publication installs their opaque owners with the complete value/companion bundle. Shared result reservation points also supply declarations for values later consumed by copy, preserving existing conversion, kernel and solve behavior without changing their numerical algorithms. Statistical parameters, product-contribution regions and other heap paths are outside this homogeneous import/copy admission. The region-bindings step in `docs/design/lc-resonance.md` owns the complete meaning-free heap and kernel migration.
//!
//! `Heap::import_symbols` accepts a predeclared bindings symbol inventory and atomically imports its centers, total bounds and optional numerical import errors. The latter must be nonnegative, finite, complete and covered by the total. Supplied import bounds remain assumptions; their separate kinds do not certify them. The returned `ValueBinding` retains a storage-neutral bindings `SymbolBinding` over opaque versioned numerical tickets. Existing convenience imports construct the same exact declarations before storage allocation. Imports require independent declarations; a derived result cannot acquire arbitrary import bytes to bypass its checked publication.
//!
//! `Heap::symbol_binding` captures a current value or alias selection. `Heap::value_by_symbol` requires its exact value symbol and selected binding, then revalidates scope, occupation, value version, selected layout, precision, schema and complete companion inventory. An alias retains its base's symbol owner. A changed version invalidates an earlier binding even when center bits agree; a foreign heap, wrong symbol or changed companion refuses. `Binding::from_symbols` consumes those same selected input bindings through the existing checked execution lease.
//!
//! `Bound::result_bindings` exposes every planned result and companion symbol before evaluation. Its `SymbolState` is Pending until the complete publication commit, so `value_by_symbol` refuses premature access. Successful publication changes all result states to Published after installing all value/companion region versions and their owners. Dropping the execution or refusing publication makes every pending plan Unavailable; reused private slot positions cannot revive it. Published records still require live ticket validation and may later become stale. No state transition exposes a partial output bundle.
//!
//! The public symbol lifecycle tests in `tests/integration/symbol_lifecycle.rs` exercise both precisions, repeated copies, numerical-error companions, pending lookup, abandoned-plan slot reuse, late atomic refusal, foreign/wrong/stale bindings, repeated alias selections and invalid import bundles. Existing numerical and acceptance work pins remain unchanged. The new symbol construction uses the bindings/CAS ledger, while owner metadata is bounded by the existing descriptor capacity and fixed three-region inventory; it introduces no numerical coefficient evaluation.

use crate::heap::identity::Key;
use crate::{
    Bound, Extent, Handle, Heap, HeapError, Layout, PublishedBundle, SchemaId, Value, ValueStamp,
    ValueView,
};
use nrz_bindings::{BindingSymbols, QuantitySymbol, SymbolBinding};
use nrz_scalar::{Precision, Scalar};
use std::{cell::Cell, rc::Rc};

/// Availability of one complete value and companion binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolState {
    /// Exact symbols and private slots exist; no numerical value is accessible.
    Pending,
    /// The complete value and companion bundle was atomically published.
    Published,
    /// Its execution ended without publishing this planned bundle.
    Unavailable,
}

/// Opaque numerical ticket; only the owning heap can resolve its live authority.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Ticket {
    /// Heap scope and occupation, never a buffer address.
    scope: u64,
    /// Selected value or companion occupation.
    key: Key,
    /// Captured base value or companion target version.
    stamp: ValueStamp,
    /// Exact selected logical entry map.
    layout: Layout,
    /// Immutable numerical schema identity.
    schema: SchemaId,
    /// Sealed numerical format.
    precision: Precision,
}

/// A selected symbol binding; access must revalidate every numerical ticket.
#[derive(Clone, Debug)]
pub struct ValueBinding<P: Scalar> {
    /// Bindings owns the immutable symbol-to-ticket association.
    exact: SymbolBinding<Ticket>,
    /// Internal numerical locator, unavailable through the public binding API.
    handle: Handle<Value, P>,
    /// Shared state follows the complete publication transaction.
    state: Rc<Cell<SymbolState>>,
}

impl<P: Scalar> ValueBinding<P> {
    /// Exact value symbol that selects this binding's meaning.
    #[must_use]
    pub const fn symbol(&self) -> &QuantitySymbol {
        self.exact.symbols().value()
    }
    /// Complete declared value and companion inventory.
    #[must_use]
    pub const fn symbols(&self) -> &BindingSymbols {
        self.exact.symbols()
    }
    /// Pending, published or explicitly unavailable state.
    #[must_use]
    pub fn state(&self) -> SymbolState {
        self.state.get()
    }
    /// Make an already completely committed import binding readable.
    pub(crate) fn publish(&self) {
        self.state.set(SymbolState::Published);
    }
    /// Internal locator used only after matching the immutable selected binding.
    pub(crate) const fn handle(&self) -> Handle<Value, P> {
        self.handle
    }
}

/// Unique transaction owner of all pending result states.
#[derive(Debug)]
pub(crate) struct Plans<P: Scalar> {
    /// Complete operation-ordered bindings, prepared before execution.
    pub(crate) values: Vec<ValueBinding<P>>,
}

impl<P: Scalar> Plans<P> {
    /// Atomically make every prepared binding readable under the exclusive lease.
    pub(crate) fn publish(&self) {
        for value in &self.values {
            value.state.set(SymbolState::Published);
        }
    }
}

impl<P: Scalar> Drop for Plans<P> {
    fn drop(&mut self) {
        for value in &self.values {
            if value.state.get() == SymbolState::Pending {
                value.state.set(SymbolState::Unavailable);
            }
        }
    }
}

impl Heap {
    /// Capture a current symbol-selected binding, including an alias's exact entry map.
    /// # Errors
    /// Refuses foreign/stale regions, missing owners or companion disagreement.
    pub fn symbol_binding<P: Scalar + 'static>(
        &self,
        handle: Handle<Value, P>,
    ) -> Result<ValueBinding<P>, HeapError> {
        let symbols = super::symbols::capture(self, handle)?;
        let value = self.value(handle)?;
        let stamp = value.stamp();
        let bound = self.selected_bound(handle)?;
        let error = self.selected_evaluation(handle)?;
        let regions = [
            Some(ticket(self.scope, handle.key, stamp, value.region())),
            value
                .bound()
                .zip(bound.as_ref())
                .map(|(handle, view)| ticket(self.scope, handle.key, stamp, view.region())),
            value
                .evaluation()
                .zip(error.as_ref())
                .map(|(handle, view)| ticket(self.scope, handle.key, stamp, view.region())),
        ];
        Ok(ValueBinding {
            exact: SymbolBinding::check(symbols, regions)?,
            handle,
            state: Rc::new(Cell::new(SymbolState::Published)),
        })
    }

    /// Resolve a value by its exact symbol and selected binding.
    /// # Errors
    /// Refuses pending/failed outputs, wrong symbols, foreign heaps and stale tickets.
    pub fn value_by_symbol<P: Scalar + 'static>(
        &self,
        symbol: &QuantitySymbol,
        binding: &ValueBinding<P>,
    ) -> Result<ValueView<'_, P>, HeapError> {
        if symbol != binding.symbol() {
            return Err(nrz_bindings::AdmissionError::ValueMismatch.into());
        }
        if binding.state() != SymbolState::Published {
            return Err(HeapError::UnavailableSymbol);
        }
        let current = self.symbol_binding(binding.handle)?;
        let value_matches = current.symbol() == binding.symbol();
        let bound_matches = current.symbols().bound() == binding.symbols().bound();
        let error_matches = current.symbols().error() == binding.symbols().error();
        if !value_matches
            || !bound_matches
            || !error_matches
            || current.exact.regions() != binding.exact.regions()
        {
            return Err(HeapError::StaleEvidence);
        }
        self.value(binding.handle)
    }
}

impl<P: Scalar + 'static, Q: Scalar + 'static> Bound<'_, P, Q> {
    /// Planned output bindings, with all result and companion symbols already declared.
    #[must_use]
    pub fn result_bindings(&self) -> &[ValueBinding<Q>] {
        &self.reservations.planned.values
    }

    /// Inspect a symbol-selected input or test a planned output's availability.
    /// # Errors
    /// Refuses pending/failed outputs and every live ticket or symbol mismatch.
    pub fn value_by_symbol<R: Scalar + 'static>(
        &self,
        symbol: &QuantitySymbol,
        binding: &ValueBinding<R>,
    ) -> Result<ValueView<'_, R>, HeapError> {
        self.heap.value_by_symbol(symbol, binding)
    }
}

/// Capture a current numerical selection without copying any payload into CAS.
fn ticket<P: Scalar + 'static>(
    scope: u64,
    key: Key,
    stamp: ValueStamp,
    region: &crate::RegionView<'_, P>,
) -> Ticket {
    Ticket {
        scope,
        key,
        stamp,
        layout: region.layout().clone(),
        schema: region.schema_id(),
        precision: P::PRECISION,
    }
}

/// Prepare exact bindings to future numerical slots before any kernel can run.
pub(crate) fn planned<P: Scalar>(
    scope: u64,
    bundle: PublishedBundle<P>,
    stamp: ValueStamp,
    extent: Extent,
    schemas: [Option<SchemaId>; 3],
    symbols: &BindingSymbols,
) -> Result<ValueBinding<P>, HeapError> {
    let keys = [
        Some(bundle.value().key),
        bundle.bound().map(|handle| handle.key),
        bundle.evaluation().map(|handle| handle.key),
    ];
    let layout = Layout::contiguous(extent)?;
    let mut regions = [None, None, None];
    for ((region, key), schema) in regions.iter_mut().zip(keys).zip(schemas) {
        match (key, schema) {
            (Some(key), Some(schema)) => {
                *region = Some(Ticket {
                    scope,
                    key,
                    stamp,
                    layout: layout.clone(),
                    schema,
                    precision: P::PRECISION,
                });
            }
            (None, None) => {}
            _ => return Err(HeapError::LengthMismatch),
        }
    }
    Ok(ValueBinding {
        exact: SymbolBinding::check(symbols.clone(), regions)?,
        handle: bundle.value(),
        state: Rc::new(Cell::new(SymbolState::Pending)),
    })
}
