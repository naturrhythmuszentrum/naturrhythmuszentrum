// SPDX-License-Identifier: AGPL-3.0-only
//! Concrete dependency snapshots are independent of storage addresses and relocation.

use nrz_scalar::Scalar;

use super::{OperationError, RegionContract};
use crate::{Companion, Handle, Heap, Layout, SchemaId, StructureScope, Value, ValueStamp};

/// Captured companion occupation, interpretation and exact target version.
#[derive(Clone, Debug)]
pub struct CompanionSnapshot<P: Scalar> {
    /// Current companion locator, including its occupation generation.
    handle: Handle<Companion, P>,
    /// Whole-base target locator.
    target: Handle<Value, P>,
    /// Target identity and content version.
    stamp: ValueStamp,
    /// Independent companion interpretation.
    schema: SchemaId,
}

impl<P: Scalar> CompanionSnapshot<P> {
    /// Companion locator; access must still resolve it through its owner.
    #[must_use]
    pub const fn handle(&self) -> Handle<Companion, P> {
        self.handle
    }
    /// Whole-base target, before applying the input snapshot's entry map.
    #[must_use]
    pub const fn target(&self) -> Handle<Value, P> {
        self.target
    }
    /// Actual target identity and version captured during binding.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.stamp
    }
    /// Immutable independently derived radius schema.
    #[must_use]
    pub const fn schema(&self) -> SchemaId {
        self.schema
    }
}

/// One operation-ordered input dependency, retaining its selected logical map.
#[derive(Clone, Debug)]
pub struct InputSnapshot<P: Scalar> {
    /// Bindings-checked value and companion symbols for the owning base.
    pub(crate) symbols: nrz_bindings::BindingSymbols,
    /// Selected reading and its complete shared-entry/proof dependency closure.
    pub(crate) selected: Option<super::live::selected::SelectedReading>,
    /// Selected value occupation, possibly an alias.
    handle: Handle<Value, P>,
    /// Actual base value identity and content version.
    stamp: ValueStamp,
    /// Exact homogeneous schema identity, before selection.
    schema: SchemaId,
    /// Selection of base entries in operation order.
    layout: Layout,
    /// Complete supported companion dependency.
    companion: Option<CompanionSnapshot<P>>,
    /// Distinct numerical-error region, when carried by a derived value.
    pub(crate) evaluation: Option<CompanionSnapshot<P>>,
    /// Captured product contribution occupations and target versions.
    pub(crate) contributions: Option<[CompanionSnapshot<P>; 3]>,
    /// Physical-input coverage is retained independently of numerical companions.
    pub(crate) coverage: crate::Coverage,
}

impl<P: Scalar> InputSnapshot<P> {
    /// Selected value occupation.
    #[must_use]
    pub const fn handle(&self) -> Handle<Value, P> {
        self.handle
    }
    /// Actual base identity and content version.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.stamp
    }
    /// Exact owning schema identity.
    #[must_use]
    pub const fn schema(&self) -> SchemaId {
        self.schema
    }
    /// Logical input map into the base value and its companion.
    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.layout
    }
    /// Captured bound target and schema, when declared coverage is present.
    #[must_use]
    pub const fn companion(&self) -> Option<&CompanionSnapshot<P>> {
        self.companion.as_ref()
    }
}

impl<P: Scalar + 'static> InputSnapshot<P> {
    /// Resolve the complete dependency set before numerical validation.
    pub(crate) fn capture(heap: &Heap, handle: Handle<Value, P>) -> Result<Self, OperationError> {
        heap.capture_joint_operation(handle)?;
        let value = heap.value(handle)?;
        let companion = value
            .bound()
            .map(|bound| {
                let view = heap.companion(bound)?;
                Ok::<_, crate::HeapError>(CompanionSnapshot {
                    handle: bound,
                    target: view.target(),
                    stamp: view.target_stamp(),
                    schema: view.region().schema_id(),
                })
            })
            .transpose()?;
        let evaluation = value
            .evaluation()
            .map(|handle| {
                let view = heap.companion(handle)?;
                Ok::<_, crate::HeapError>(CompanionSnapshot {
                    handle,
                    target: view.target(),
                    stamp: view.target_stamp(),
                    schema: view.region().schema_id(),
                })
            })
            .transpose()?;
        Ok(Self {
            symbols: super::symbols::capture(heap, handle)?,
            selected: heap.symbols.selection(handle.key).cloned(),
            handle,
            stamp: if heap.symbols.selection(handle.key).is_some() {
                heap.symbols.cover(heap.base_handle(handle)?.key)?.stamp
            } else {
                value.stamp()
            },
            schema: value.region().schema_id(),
            layout: value.region().layout().clone(),
            companion,
            evaluation,
            coverage: value.coverage(),
            contributions: capture_contributions(heap, value.contributions)?,
        })
    }

    /// Recheck concrete schema, finite payloads and structural obligations for this slot.
    pub(crate) fn validate(
        &self,
        heap: &Heap,
        contract: &RegionContract,
        slot: usize,
    ) -> Result<(), OperationError> {
        let value = heap.value(self.handle)?;
        let region = value.region();
        if let Some(selected) = &self.selected {
            selected.validate(heap)?;
        }
        let stamp_matches = self.selected.as_ref().map_or_else(
            || value.stamp() == self.stamp,
            |_| value.stamp().id() == self.stamp.id(),
        );
        if !stamp_matches
            || region.schema_id() != self.schema
            || region.layout() != &self.layout
            || region.extent() != contract.shape().extent()
        {
            return Err(OperationError::InputMismatch { slot });
        }
        self.symbols
            .value()
            .check_descriptor(contract.descriptor())
            .map_err(|_| OperationError::InputMismatch { slot })?;
        let current = super::symbols::capture(heap, self.handle)?;
        if current.value() != self.symbols.value()
            || current.bound() != self.symbols.bound()
            || current.error() != self.symbols.error()
        {
            return Err(OperationError::InputMismatch { slot });
        }
        let bounds = heap.selected_bound(self.handle)?;
        for (entry, meaning) in contract.shape().entries().iter().enumerate() {
            let row = entry % region.extent().rows();
            let column = entry / region.extent().rows();
            let center = region.get(row, column)?;
            let radius = bounds
                .as_ref()
                .map(|bound| bound.region().get(row, column))
                .transpose()?;
            check_entry(
                center,
                radius,
                meaning.is_zero(),
                contract.shape().structure_scope(),
                slot,
                entry,
            )?;
        }
        Ok(())
    }
}

/// Identity normalization has no rounding; structural zeros still need numerical proof.
fn check_entry<P: Scalar>(
    center: P,
    radius: Option<P>,
    zero: bool,
    scope: StructureScope,
    slot: usize,
    entry: usize,
) -> Result<(), OperationError> {
    if !center.is_finite() {
        return Err(crate::HeapError::NonFinite { index: entry }.into());
    }
    if radius.is_some_and(|value| !value.is_finite() || value < P::ZERO) {
        return Err(crate::HeapError::InvalidBound { index: entry }.into());
    }
    if zero
        && (center != P::ZERO
            || (scope == StructureScope::WholeFamily
                && radius.is_some_and(|value| value != P::ZERO)))
    {
        return Err(OperationError::StructureMismatch { slot, entry });
    }
    Ok(())
}

/// Capture all product regions as an all-or-none dependency inventory.
fn capture_contributions<P: Scalar + 'static>(
    heap: &Heap,
    handles: Option<[Handle<Companion, P>; 3]>,
) -> Result<Option<[CompanionSnapshot<P>; 3]>, OperationError> {
    let Some(handles) = handles else {
        return Ok(None);
    };
    let [first, second, interaction] = handles.map(|handle| {
        let view = heap.companion(handle)?;
        Ok::<_, OperationError>(CompanionSnapshot {
            handle,
            target: view.target(),
            stamp: view.target_stamp(),
            schema: view.region().schema_id(),
        })
    });
    Ok(Some([first?, second?, interaction?]))
}
