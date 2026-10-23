// SPDX-License-Identifier: AGPL-3.0-only
//! Live entry coverage, immutable exact evidence and versioned numerical selections.

mod admission;
mod bounded;
pub(crate) mod companions;
mod coordinates;
mod copies;

pub use copies::CopyDiscrepancy;
mod dependencies;

pub use coordinates::{CoordinateContract, CoordinateError, CoordinateNorm, CoordinateReading};
mod import;
pub(crate) mod invalidation;
pub(crate) mod results;
pub(crate) mod selected;
mod write;

use crate::{Extent, HeapError, ReadingAdapter, ReadingAdapterError, ReadingCoverage, SymbolState};
use nrz_bindings::readings::{
    PhysicalReading,
    algebra::{Cas, Fault, readings::DependencyInventory},
};
use nrz_scalar::{Precision, Scalar};
use std::{cell::Cell, marker::PhantomData, rc::Rc};

/// Dependency evidence selected before a numerical allocation is admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyPolicy {
    /// Cache exact reading and proof inventories and retain entry versions.
    Entries,
    /// Invalidate the whole family whenever any participating base changes.
    WholeBase,
}

/// A complete finite numerical region supplied to a checked reading import.
#[derive(Clone, Debug)]
pub struct ReadingRegion<P: Scalar> {
    /// Literal logical shape in column order.
    pub extent: Extent,
    /// Complete represented payload; all entries must be covered before allocation.
    pub values: Vec<P>,
}

/// A live admission or mutation refusal, before any partial numerical publication.
#[derive(Clone, Debug, thiserror::Error)]
pub enum LiveBindingError {
    /// Outward verification of stored-copy consistency could not be represented.
    #[error("stored copy arithmetic: {0}")]
    Arithmetic(#[from] nrz_scalar::ArithmeticFailure),
    /// Differing centers are not provably covered by the two selected numerical radii.
    #[error("stored copy discrepancy exceeds its declared numerical coverage")]
    UncoveredCopy,
    /// Numerical shape, version, precision, finiteness or allocation check failed.
    #[error("numerical reading: {0}")]
    Heap(#[from] HeapError),
    /// Total coverage or a particular overlap equation does not match.
    #[error("reading coverage: {0}")]
    Adapter(#[from] ReadingAdapterError),
    /// Exact dependency authentication or resource accounting failed.
    #[error("reading dependencies: {0}")]
    Exact(#[from] Fault),
}

/// An immutable selected interpretation; each access resolves its entry versions.
#[derive(Clone, Debug)]
pub struct ReadingBinding<P: Scalar> {
    /// Shared complete declaration, storage map and proof inventory.
    pub(crate) group: Rc<Group>,
    /// Exact selected reading, never chosen by an equal schema or display name.
    pub(crate) index: usize,
    /// Sealed scalar format.
    marker: PhantomData<P>,
}

impl<P: Scalar> ReadingBinding<P> {
    /// Exact physical reading, retaining original selections, maps and family.
    #[must_use]
    pub fn reading(&self) -> &PhysicalReading {
        self.adapter().reading()
    }
    /// Complete representation, including a no-storage constancy witness for None.
    #[must_use]
    pub fn adapter(&self) -> &ReadingAdapter<usize> {
        &self.group.coverage.readings()[self.index]
    }
    /// Exact companion definitions with their complete ordered numerical target maps.
    #[must_use]
    pub fn companions(&self) -> &[crate::CompanionAdapter<usize>] {
        &self.group.companions
    }
    /// Differing rounded copies in this exact family, with their selected error parameters.
    pub fn copy_discrepancies(&self) -> impl Iterator<Item = &CopyDiscrepancy> {
        self.group
            .copies
            .iter()
            .filter(|copy| copy.family() == self.reading().family().node())
    }
    /// Transaction availability; published bindings still require live version checks.
    #[must_use]
    pub fn state(&self) -> SymbolState {
        self.group.states[self.index].get()
    }
}

/// Complete result of a single checked reading import.
#[derive(Clone, Debug)]
pub struct ReadingImport<P: Scalar> {
    /// All declared readings, including components sharing the same numerical entries.
    pub(crate) bindings: Vec<ReadingBinding<P>>,
}

impl<P: Scalar> ReadingImport<P> {
    /// Every admitted reading in its original declaration order.
    #[must_use]
    pub fn bindings(&self) -> &[ReadingBinding<P>] {
        &self.bindings
    }
}

/// One actual numerical occupation and the versions captured before publication.
#[derive(Clone, Debug)]
pub(crate) struct RegionTicket {
    /// Exact heap scope, occupation and precision.
    pub(crate) scope: u64,
    /// Exact descriptor occupation and generation.
    pub(crate) key: crate::heap::identity::Key,
    /// Sealed numerical format.
    pub(crate) precision: Precision,
    /// Full immutable numerical map; individual readers retain their own entry order.
    pub(crate) layout: crate::Layout,
    /// Original allocation identity, independent of later base-wide content revisions.
    pub(crate) identity: crate::ValueId,
    /// Sponsor revision; coarse replacements change this independently of entry versions.
    pub(crate) generation: crate::ValueVersion,
    /// Exact immutable numerical shape identity.
    pub(crate) schema: usize,
    /// Version of every originally covered entry.
    pub(crate) versions: Vec<u64>,
}

/// A proof and the exact readers whose availability depends on it.
#[derive(Debug)]
pub(crate) struct ProofDependency {
    /// Cached authenticated witness closure.
    pub(crate) inventory: DependencyInventory,
    /// All exact reading indices that use this proof.
    pub(crate) readers: Vec<usize>,
}

/// Shared immutable exact plan, owned by the same inventory as homogeneous covers.
#[derive(Debug)]
pub(crate) struct Group {
    /// Owning heap identity, also checked for no-storage readings.
    pub(crate) scope: u64,
    /// Sealed numerical format.
    pub(crate) precision: Precision,
    /// Exact graph retained for authenticated write-intersection queries.
    pub(crate) graph: Rc<Cas>,
    /// Complete resolved value-entry universe and every checked overlap.
    pub(crate) coverage: ReadingCoverage<usize>,
    /// Actual region occupations, maps and original entry versions.
    pub(crate) tickets: Vec<RegionTicket>,
    /// Transaction and structural validity of every selected reading.
    pub(crate) states: Vec<Cell<SymbolState>>,
    /// One cached dependency inventory per immutable reading, or explicit coarse fallback.
    pub(crate) reads: Option<Vec<DependencyInventory>>,
    /// Cached closed-recipe restrictions for exactly identity-coordinate readings.
    pub(crate) homogeneous: Vec<Option<nrz_bindings::BindingSymbols>>,
    /// Complete reconstruction and overlap proof dependencies.
    pub(crate) proofs: Vec<ProofDependency>,
    /// Exact declared companion definitions and all ordered target maps.
    pub(crate) companions: Vec<crate::CompanionAdapter<usize>>,
    /// Cached dependency inventories of immutable companion definitions.
    pub(crate) companion_reads: Vec<DependencyInventory>,
    /// Verified per-copy discrepancies and their exact selected error dependencies.
    pub(crate) copies: Vec<CopyDiscrepancy>,
    /// Native result companion tickets retained before general map lowering.
    pub(crate) native: Vec<companions::NativeCompanion>,
}

impl Group {
    /// Abandon every planned reader when its reservation is dropped.
    pub(crate) fn unavailable(&self) {
        for state in &self.states {
            state.set(SymbolState::Unavailable);
        }
    }
    /// Availability changes only after the complete numerical commit.
    pub(crate) fn publish(&self) {
        for state in &self.states {
            state.set(SymbolState::Published);
        }
    }
}

/// Create typed references only to indices present in the sealed checked plan.
pub(crate) fn bindings<P: Scalar>(group: &Rc<Group>) -> ReadingImport<P> {
    ReadingImport {
        bindings: (0..group.states.len())
            .map(|index| ReadingBinding {
                group: Rc::clone(group),
                index,
                marker: PhantomData,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests;
