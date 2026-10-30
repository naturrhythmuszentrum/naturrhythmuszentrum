// SPDX-License-Identifier: AGPL-3.0-only
//! Exact physical tuple snapshots retain mathematical selection and reconstruction separately.

use super::{DecisionError, LatticeAttachment, LatticeDecisionCase, LatticeIndexKind};
use crate::{Descriptor, decision::storage};
use nrz_cas::{
    Cas, ExactConstant, Rational,
    limits::{Budget, BudgetUsage},
    proof::VerifiedWitness,
};
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};

/// An exact selected physical tuple, independent of executable representation and capacity.
#[derive(Clone, Debug)]
pub struct SelectedLattice(Rc<Data>);

/// Complete mathematical and physical evidence lives with every exact output.
#[derive(Debug)]
struct Data {
    /// Guarded mathematical authority.
    graph: Arc<Mutex<Cas>>,
    /// Original physical axes, maps, sources and declared index kinds.
    case: LatticeDecisionCase,
    /// Declaration identity authenticated before selected publication.
    attachment: LatticeAttachment,
    /// Exact mathematical cell membership and original enclosure dependencies.
    selection: VerifiedWitness,
    /// Independent original-coordinate reconstruction theorem.
    reconstruction: VerifiedWitness,
    /// Exact Integer values; no machine conversion has occurred.
    indices: Vec<Rational>,
    /// Exact coherent physical representatives, preserving each original kind and role.
    representatives: Vec<Rc<ExactConstant>>,
    /// Unique exact normalization and restoration judgments.
    preparations: super::physical::PhysicalPreparations,
    /// Checked physical B*k differences reused by optional site construction.
    displacements: Vec<Rc<ExactConstant>>,
    /// Caller ledger before and after complete physical admission.
    usage: [BudgetUsage; 2],
    /// Newly owned exact values and native witness allowance.
    bytes: u64,
}

/// One borrowed exact physical output with its declared units, kind and affine role.
#[derive(Clone, Copy, Debug)]
pub struct LatticeCoordinate<'a> {
    /// Exact coherent value; decimal materialization is a different consumer.
    value: &'a ExactConstant,
    /// Original coherent physical coordinate descriptor.
    descriptor: &'a Descriptor,
}

impl LatticeCoordinate<'_> {
    /// Exact physical value in the descriptor's coherent units.
    #[must_use]
    pub const fn exact(&self) -> &ExactConstant {
        self.value
    }

    /// Declared physical kind and affine role, including angle representatives.
    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        self.descriptor
    }
}

impl SelectedLattice {
    /// Publish only after all physical values, proof ownership and identity checks succeed.
    pub(super) fn new(
        graph: Arc<Mutex<Cas>>,
        case: LatticeDecisionCase,
        attachment: LatticeAttachment,
        values: super::outputs::Values,
    ) -> Self {
        Self(Rc::new(Data {
            graph,
            case,
            attachment,
            selection: values.selection,
            reconstruction: values.reconstruction,
            indices: values.indices,
            representatives: values.representatives,
            preparations: values.preparations,
            displacements: values.displacements,
            usage: values.usage,
            bytes: values.bytes,
        }))
    }

    /// Ordered mathematical Integers with no machine-capacity restriction.
    #[must_use]
    pub fn indices(&self) -> &[Rational] {
        &self.0.indices
    }

    /// Predeclared physical index meaning at the same original column position.
    #[must_use]
    pub fn index_kind(&self, coordinate: usize) -> Option<&LatticeIndexKind> {
        self.case().specification().indices.get(coordinate)
    }

    /// Exact representative in the original physical frame, retaining its kind and units.
    #[must_use]
    pub fn representative(&self, coordinate: usize) -> Option<LatticeCoordinate<'_>> {
        self.coordinate(&self.0.representatives, coordinate)
    }

    /// Complete original physical question and source ownership.
    #[must_use]
    pub fn case(&self) -> &LatticeDecisionCase {
        &self.0.case
    }

    /// Original declaration attachment, distinct from the mathematical proof.
    #[must_use]
    pub fn attachment(&self) -> &LatticeAttachment {
        &self.0.attachment
    }

    /// Complete selected-cell theorem, including its convention and reduction/enclosure evidence.
    #[must_use]
    pub fn witness(&self) -> &VerifiedWitness {
        &self.0.selection
    }

    /// Mathematical q = B*k + r theorem; the case retains the exact physical restoration.
    #[must_use]
    pub fn reconstruction(&self) -> &VerifiedWitness {
        &self.0.reconstruction
    }

    /// Hold the retained graph guard across a complete inspection or judgment attempt.
    ///
    /// Pass this guard into nested checks instead of acquiring it again.
    /// # Errors
    /// Refuses a poisoned graph owner.
    pub fn cas(&self) -> Result<MutexGuard<'_, Cas>, DecisionError> {
        self.0
            .graph
            .lock()
            .map_err(|_| DecisionError::GraphPoisoned)
    }

    /// Unique checked output preparations retained within this selected owner.
    #[must_use]
    pub fn preparation_usage(&self) -> super::LatticePreparationUsage {
        self.0.preparations.usage()
    }

    /// Caller ledger snapshots around complete physical admission.
    #[must_use]
    pub fn usage(&self) -> [BudgetUsage; 2] {
        self.0.usage
    }

    /// Newly owned exact values and native witnesses; declared sources/maps remain shared.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }

    /// Pair one exact coordinate with its retained physical meaning without copying descriptors.
    fn coordinate<'a>(
        &'a self,
        values: &'a [Rc<ExactConstant>],
        coordinate: usize,
    ) -> Option<LatticeCoordinate<'a>> {
        let value = values.get(coordinate)?;
        let descriptor = self
            .case()
            .specification()
            .frame
            .entries()
            .get(coordinate)?
            .descriptor();
        Some(LatticeCoordinate { value, descriptor })
    }

    /// Construct the optional original-coordinate site phi+B*k from exact retained outputs.
    ///
    /// Cell membership does not claim that the target equals this site.
    /// # Errors
    /// Refuses exhausted exact arithmetic or retained-output resources without changing this tuple.
    pub fn site(&self, budget: &mut Budget) -> Result<LatticeSite, DecisionError> {
        let mut values = Vec::new();
        let mut cache = std::collections::BTreeMap::new();
        let count = self.indices().len();
        let mut bytes = u64::try_from(count)
            .map_err(|_| DecisionError::Capacity)?
            .saturating_mul(128)
            .saturating_add(256);
        storage::retained(bytes, budget)?;
        budget.work(bytes, storage::AT)?;
        values
            .try_reserve_exact(count)
            .map_err(|_| DecisionError::Capacity)?;
        for (axis, displacement) in self
            .case()
            .specification()
            .axes
            .iter()
            .zip(&self.0.displacements)
        {
            budget.work(1, storage::AT)?;
            let key = (
                Rc::as_ptr(displacement).addr(),
                std::ptr::from_ref(axis.offset.exact()).addr(),
            );
            let value = if let Some(value) = cache.get(&key) {
                Rc::clone(value)
            } else {
                let value = if axis.offset.exact().is_zero() {
                    Rc::clone(displacement)
                } else {
                    let value =
                        Rc::new(budget.with_retained_payload(bytes, storage::AT, |budget| {
                            displacement.add(axis.offset.exact(), budget)
                        })?);
                    bytes = bytes
                        .checked_add(storage::constant_bytes(&value))
                        .ok_or(DecisionError::Capacity)?;
                    storage::retained(bytes, budget)?;
                    value
                };
                cache.insert(key, Rc::clone(&value));
                value
            };
            values.push(value);
        }
        Ok(LatticeSite(Rc::new(SiteData {
            selected: self.clone(),
            values,
            bytes,
        })))
    }
}

/// Optional exact physical site, retaining the selected tuple and its complete proof.
#[derive(Clone, Debug)]
pub struct LatticeSite(Rc<SiteData>);

/// Site coordinates own exact constants while selection and physical meanings remain shared.
#[derive(Debug)]
struct SiteData {
    /// Complete original selected question and reconstruction.
    selected: SelectedLattice,
    /// Exact physical site coordinates, never rounded values.
    values: Vec<Rc<ExactConstant>>,
    /// Newly owned metadata and constant payload.
    bytes: u64,
}

impl LatticeSite {
    /// One exact site coordinate in the original declared physical frame.
    #[must_use]
    pub fn coordinate(&self, coordinate: usize) -> Option<LatticeCoordinate<'_>> {
        self.0.selected.coordinate(&self.0.values, coordinate)
    }

    /// Complete selected tuple; a site carries no separate equality-to-target claim.
    #[must_use]
    pub fn selection(&self) -> &SelectedLattice {
        &self.0.selected
    }

    /// Newly owned site metadata and exact constants.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}
