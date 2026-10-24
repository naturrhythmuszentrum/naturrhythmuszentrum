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
    /// Exact selected reading with per-entry and structural proof versions.
    pub(crate) selected: Option<crate::binding::live::selected::SelectedReading>,
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
    /// All three current product contribution targets and immutable schemas.
    pub(crate) contributions: Option<[BoundDependency; 3]>,
    /// Captured physical versus represented input coverage.
    pub(crate) coverage: crate::Coverage,
}

/// Exact independent companion dependency; no scalar carries this evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    /// Compare actual value identities, selections and versions, independently of alias descriptors.
    /// The globally unique value identity names the base occupation; aliases cannot create new sources.
    pub(crate) fn same_source(&self, other: &Self) -> bool {
        let selected = match (&self.selected, &other.selected) {
            (Some(left), Some(right)) => {
                left.index == right.index && std::rc::Rc::ptr_eq(&left.group, &right.group)
            }
            (None, None) => true,
            _ => false,
        };
        selected
            && self.scope == other.scope
            && self.stamp == other.stamp
            && self.precision == other.precision
            && self.schema == other.schema
            && self.layout == other.layout
            && self.companion == other.companion
            && self.evaluation == other.evaluation
            && self.contributions == other.contributions
            && self.coverage == other.coverage
    }

    /// Capture a precision-erased graph relation without converting any numerical data.
    pub(crate) fn from_snapshot<P: Scalar>(input: &InputSnapshot<P>) -> Self {
        Self {
            scope: input.handle().scope,
            key: input.handle().key,
            stamp: input.stamp(),
            selected: input.selected.clone(),
            precision: P::PRECISION,
            schema: input.schema(),
            layout: input.layout().clone(),
            evaluation: input.evaluation.as_ref().map(|bound| BoundDependency {
                key: bound.handle().key,
                target: bound.target().key,
                stamp: bound.stamp(),
                schema: bound.schema(),
            }),
            contributions: input.contributions.as_ref().map(|parts| {
                parts.each_ref().map(|bound| BoundDependency {
                    key: bound.handle().key,
                    target: bound.target().key,
                    stamp: bound.stamp(),
                    schema: bound.schema(),
                })
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
    pub fn carries_bound(&self) -> bool {
        self.companion.is_some()
            || self.selected.as_ref().is_some_and(|reading| {
                reading
                    .radius(nrz_bindings::CompanionKind::DeterministicBound)
                    .is_ok_and(|value| value.is_some())
            })
    }

    /// Original physical reading selected by this dependency, when it came from general storage.
    #[must_use]
    pub fn reading(&self) -> Option<&nrz_bindings::readings::PhysicalReading> {
        self.selected
            .as_ref()
            .map(crate::binding::live::selected::SelectedReading::reading)
    }
    /// Every selected original companion with its exact ordered targets and source lineage.
    pub fn reading_companions(&self) -> impl Iterator<Item = &crate::CompanionAdapter<usize>> {
        self.selected
            .iter()
            .flat_map(crate::binding::live::selected::SelectedReading::companions)
    }
    /// Every actual repeated copy in the original family and its retained discrepancy.
    pub fn copy_discrepancies(&self) -> impl Iterator<Item = &crate::CopyDiscrepancy> {
        self.selected
            .iter()
            .flat_map(crate::binding::live::selected::SelectedReading::copies)
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
            contributions,
            coverage,
        } = owner.meaning
        else {
            return Err(HeapError::CategoryMismatch);
        };
        if let Some(selected) = &self.selected {
            selected.validate(heap)?;
        }
        let stamp_matches = self
            .selected
            .as_ref()
            .map_or(stamp == self.stamp, |_| stamp.id() == self.stamp.id());
        if !stamp_matches
            || bound != self.companion.as_ref().map(|value| value.key)
            || evaluation != self.evaluation.as_ref().map(|value| value.key)
            || contributions
                != self
                    .contributions
                    .as_ref()
                    .map(|parts| parts.each_ref().map(|part| part.key))
            || self
                .selected
                .as_ref()
                .map_or(coverage, |selected| selected.coverage(coverage))
                != self.coverage
        {
            return Err(HeapError::StaleEvidence);
        }
        heap.validate_statistical_roots(self.stamp)?;
        validate_region(heap, owner, self.precision)?;
        self.layout
            .validate(heap.owning_region(owner)?.length, heap.limits.layout_work)?;
        if let Some(parts) = &self.contributions {
            for part in parts {
                part.validate(heap, base, self.precision)?;
            }
        }
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
            selected: None,
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
            contributions: if let Some(handles) = bundle.contributions {
                let schema = bound_schema.ok_or(crate::OperationError::CandidateContract)?;
                Some(handles.map(|handle| BoundDependency {
                    key: handle.key,
                    target: bundle.value().key,
                    stamp: output.stamp(),
                    schema,
                }))
            } else {
                None
            },
            coverage: output.coverage(),
            companion,
        })
    }
}
