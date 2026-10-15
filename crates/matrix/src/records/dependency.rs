// SPDX-License-Identifier: AGPL-3.0-only
//! Precision-erased dependency keys retain every concrete evidential relation.

use nrz_scalar::{Precision, Scalar};

use crate::heap::identity::Key;
use crate::heap::records::{Location, Meaning};
use crate::{Heap, HeapError, InputSnapshot, Layout, SchemaId, ValueStamp};

/// A stored value dependency, with exact occupation, selected entries and companion target.
#[derive(Clone, Debug)]
pub struct Dependency {
    /// Owner scope, checked before any slot access.
    pub(crate) scope: u64,
    /// Selected descriptor occupation, possibly an alias.
    pub(crate) key: Key,
    /// Base value identity and content version.
    pub(crate) stamp: ValueStamp,
    /// Sealed numerical format.
    pub(crate) precision: Precision,
    /// Owning homogeneous interpretation.
    pub(crate) schema: SchemaId,
    /// Exact logical entry map, stable across compaction.
    pub(crate) layout: Layout,
    /// Captured companion occupation, target occupation/version and separate schema.
    pub(crate) companion: Option<BoundDependency>,
    /// Separate numerical error occupation and exact target dependency.
    pub(crate) evaluation: Option<BoundDependency>,
    /// Captured physical versus represented input coverage.
    pub(crate) coverage: crate::Coverage,
}

/// Exact independent companion dependency; no scalar carries this evidence.
#[derive(Clone, Debug)]
pub(crate) struct BoundDependency {
    /// Captured companion occupation, including generation.
    key: Key,
    /// Whole-base target occupation.
    target: Key,
    /// Exact target identity and content version.
    stamp: ValueStamp,
    /// Companion's own immutable interpretation.
    schema: SchemaId,
}

impl Dependency {
    /// Capture a precision-erased graph relation without converting any numerical data.
    pub(crate) fn from_snapshot<P: Scalar>(input: &InputSnapshot<P>) -> Self {
        Self {
            scope: input.handle().scope,
            key: input.handle().key,
            stamp: input.stamp(),
            precision: P::PRECISION,
            schema: input.schema(),
            layout: input.layout().clone(),
            evaluation: input.evaluation.as_ref().map(|bound| BoundDependency {
                key: bound.handle().key,
                target: bound.target().key,
                stamp: bound.stamp(),
                schema: bound.schema(),
            }),
            coverage: input.coverage,
            companion: input.companion().map(|bound| BoundDependency {
                key: bound.handle().key,
                target: bound.target().key,
                stamp: bound.stamp(),
                schema: bound.schema(),
            }),
        }
    }

    /// Actual mathematical input identity and content version.
    #[must_use]
    pub const fn stamp(&self) -> ValueStamp {
        self.stamp
    }
    /// Exact immutable physical schema identity.
    #[must_use]
    pub const fn schema(&self) -> SchemaId {
        self.schema
    }
    /// Selected logical entries into the base value and its bound.
    #[must_use]
    pub const fn layout(&self) -> &Layout {
        &self.layout
    }
    /// Original sealed precision, without a numerical cast.
    #[must_use]
    pub const fn precision(&self) -> Precision {
        self.precision
    }
    /// Whether declared-bound evidence forms part of this exact dependency key.
    #[must_use]
    pub const fn carries_bound(&self) -> bool {
        self.companion.is_some()
    }

    /// Check raw descriptors without recursively entering public provenance access.
    pub(crate) fn validate(&self, heap: &Heap) -> Result<(), HeapError> {
        if self.scope != heap.scope {
            return Err(HeapError::ForeignHandle);
        }
        let selected = heap.descriptors.get(self.key)?;
        if selected.schema != self.schema.index || selected.layout != self.layout {
            return Err(HeapError::StaleEvidence);
        }
        let base = match selected.meaning {
            Meaning::Value { .. } => self.key,
            Meaning::Alias { base } => base,
            Meaning::Bound { .. } | Meaning::Statistical => {
                return Err(HeapError::CategoryMismatch);
            }
        };
        let owner = heap.descriptors.get(base)?;
        let Meaning::Value {
            stamp,
            bound,
            evaluation,
            coverage,
        } = owner.meaning
        else {
            return Err(HeapError::CategoryMismatch);
        };
        if stamp != self.stamp
            || bound != self.companion.as_ref().map(|value| value.key)
            || evaluation != self.evaluation.as_ref().map(|value| value.key)
            || coverage != self.coverage
        {
            return Err(HeapError::StaleEvidence);
        }
        validate_region(heap, owner, self.precision)?;
        self.layout
            .validate(heap.owning_region(owner)?.length, heap.limits.layout_work)?;
        if let Some(companion) = &self.evaluation {
            companion.validate(heap, base, self.precision)?;
        }
        if let Some(companion) = &self.companion {
            companion.validate(heap, base, self.precision)?;
        }
        Ok(())
    }
}

impl BoundDependency {
    /// Both forward and reverse companion edges must still name the captured target.
    fn validate(&self, heap: &Heap, base: Key, precision: Precision) -> Result<(), HeapError> {
        let record = heap.descriptors.get(self.key)?;
        if base != self.target
            || record.schema != self.schema.index
            || self.schema.scope != heap.scope
            || !matches!(record.meaning, Meaning::Bound { target, stamp } if target == self.target && stamp == self.stamp)
        {
            return Err(HeapError::StaleEvidence);
        }
        validate_region(heap, record, precision)
    }
}

/// Compaction changes regions and epochs, so resolve current indirection on every access.
fn validate_region(
    heap: &Heap,
    record: &crate::heap::records::Record,
    precision: Precision,
) -> Result<(), HeapError> {
    let Location::Owned(region) = record.location else {
        return Err(HeapError::InvalidLayout);
    };
    let expected = match precision {
        Precision::F32 => crate::storage::Format::Binary32,
        Precision::F64 => crate::storage::Format::Binary64,
    };
    if region.format != expected {
        return Err(HeapError::PrecisionMismatch);
    }
    heap.storage.resolve(region)?;
    record
        .layout
        .validate(region.length, heap.limits.layout_work)
}

impl Dependency {
    /// Construct a graph target from the already checked invisible output reservation.
    pub(crate) fn reserved<P: Scalar>(
        bundle: crate::PublishedBundle<P>,
        output: crate::OutputReservation,
        bound_schema: Option<SchemaId>,
    ) -> Result<Self, crate::OperationError> {
        let companion = match (bundle.bound(), bound_schema) {
            (Some(handle), Some(schema)) => Some(BoundDependency {
                key: handle.key,
                target: bundle.value().key,
                stamp: output.stamp(),
                schema,
            }),
            (None, None) => None,
            _ => return Err(crate::OperationError::CandidateContract),
        };
        Ok(Self {
            scope: bundle.value().scope,
            key: bundle.value().key,
            stamp: output.stamp(),
            precision: P::PRECISION,
            schema: output.schema(),
            layout: Layout::contiguous(output.extent())?,
            evaluation: match (bundle.evaluation(), bound_schema) {
                (Some(handle), Some(schema)) => Some(BoundDependency {
                    key: handle.key,
                    target: bundle.value().key,
                    stamp: output.stamp(),
                    schema,
                }),
                (None, _) => None,
                _ => return Err(crate::OperationError::CandidateContract),
            },
            coverage: output.coverage(),
            companion,
        })
    }
}
