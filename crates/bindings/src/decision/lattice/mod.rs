// SPDX-License-Identifier: AGPL-3.0-only
//! Physical lattice questions compose original sources with existing exact coordinate maps.

mod correspondence;
mod inputs;
mod preparation;
mod storage;

pub use inputs::LatticeInputs;
pub use preparation::LatticePreparationUsage;

use super::{DecisionCoordinate, DecisionError};
use crate::{Descriptor, Kind, readings::CoordinateNormalization};
use nrz_cas::{
    Cas, Handle,
    lattice::{
        CheckedLatticeBasis, CheckedLatticeIndexDomain, LatticeConvention, LatticeIndexQuestion,
    },
    limits::Budget,
};
use std::rc::Rc;

/// Physical meaning of one exact Integer coordinate, independent of machine capacity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LatticeIndexKind {
    /// Signed complete-turn winding in the named physical phase.
    SignedWinding(String),
    /// Signed translation along the named original crystal basis column.
    CrystalTranslation(String),
    /// Signed position on the named quantization grid axis.
    QuantizationIndex(String),
    /// An explicitly counted thing with an admitted nonnegative mathematical domain.
    Count(Box<Descriptor>),
}

/// The metric is explicit in the normalized frame, including its retained references.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LatticeMetric {
    /// Euclidean distance after the declared physical normalization, with original-index ties.
    EuclideanNormalized,
}

/// One ordered physical coordinate and all original expressions contributing to its row.
#[derive(Clone, Debug)]
pub struct LatticeAxis {
    /// Physical coordinate name, unique in this ordered frame.
    pub name: String,
    /// Original target point or difference in this physical kind.
    pub target: DecisionCoordinate,
    /// Original affine offset, with the same physical role as the target.
    pub offset: DecisionCoordinate,
    /// Ordered physical differences contributed by the original basis columns.
    pub basis: Vec<DecisionCoordinate>,
}

/// Candidate-free physical declaration over already admitted mathematical prerequisites.
#[derive(Clone, Debug)]
pub struct LatticeDecisionSpec {
    /// Checked original exact basis and both affine inverse identities, shared with its owner.
    pub basis: Rc<CheckedLatticeBasis>,
    /// Checked ordered index ranges and all original parameter bindings.
    pub index: Rc<CheckedLatticeIndexDomain>,
    /// Exact physical normalization/restoration, ordered references and point origins.
    pub frame: CoordinateNormalization,
    /// Ordered original physical rows, including every zero basis contribution.
    pub axes: Vec<LatticeAxis>,
    /// Meaning of every original Integer basis coordinate, in column order.
    pub indices: Vec<LatticeIndexKind>,
    /// Original mathematical target column in the declared normalized frame.
    pub target: Handle,
    /// Original cell and tie convention; equal tuples cannot identify conventions.
    pub convention: LatticeConvention,
    /// Required for nearest cells and absent for floor cells.
    pub metric: Option<LatticeMetric>,
}

/// Immutable complete physical question, with no proposed or selected Integer tuple.
#[derive(Clone, Debug)]
pub struct LatticeDecisionCase(Rc<Data>);

/// Original physical question identity; equal centers never authenticate another declaration.
#[derive(Clone, Debug)]
pub struct LatticeAttachment(Rc<Data>);

/// Shared checked correspondence and its newly owned metadata allowance.
#[derive(Debug)]
struct Data {
    /// Complete original mathematical and physical identities.
    specification: LatticeDecisionSpec,
    /// Unique checked constants and restoration judgments shared by all occurrences.
    preparations: preparation::Preparations,
    /// Newly owned names, vectors and index descriptors; other owners remain shared.
    bytes: u64,
}

impl LatticeDecisionCase {
    /// Check closed physical target, offset and basis differences against the exact CAS question.
    ///
    /// Every row restores through its original reference and point origin. A basis
    /// difference uses only the reference scale. No tuple or cover is inferred.
    /// # Errors
    /// Refuses changed physical correspondences, malformed axes/kinds/metrics, foreign
    /// mathematical prerequisites and caller resource exhaustion without publication.
    pub fn declare(
        cas: &Cas,
        specification: LatticeDecisionSpec,
        budget: &mut Budget,
    ) -> Result<Self, DecisionError> {
        let bytes = storage::specification(&specification, budget)?;
        let preparations = budget.with_retained_payload(bytes, super::storage::AT, |budget| {
            correspondence::check(cas, &specification, budget)
        })?;
        let bytes = bytes.saturating_add(preparations.usage().bytes);
        super::storage::retained(bytes, budget)?;
        Ok(Self(Rc::new(Data {
            specification,
            preparations,
            bytes,
        })))
    }

    /// Complete retained physical declaration, including ordered source occurrences.
    #[must_use]
    pub fn specification(&self) -> &LatticeDecisionSpec {
        &self.0.specification
    }

    /// Exact admitted CAS index question; physical meaning is retained by this owner.
    #[must_use]
    pub fn question(&self) -> &LatticeIndexQuestion {
        self.0.specification.index.question()
    }

    /// Capture the immutable declaration identity without copying sources or proof bytes.
    #[must_use]
    pub fn attachment(&self) -> LatticeAttachment {
        LatticeAttachment(Rc::clone(&self.0))
    }

    /// Authenticate the entire declaration, including equal-valued independent sources.
    /// # Errors
    /// Refuses a different basis, offset, source, frame, index kind or convention declaration.
    pub fn validate_attachment(
        &self,
        attachment: &LatticeAttachment,
        budget: &mut Budget,
    ) -> Result<(), DecisionError> {
        budget.work(1, super::storage::AT)?;
        if Rc::ptr_eq(&self.0, &attachment.0) {
            Ok(())
        } else {
            Err(DecisionError::Attachment)
        }
    }

    /// Unique prepared judgments retained by this operation, excluding original shared owners.
    #[must_use]
    pub fn preparation_usage(&self) -> LatticePreparationUsage {
        self.0.preparations.usage()
    }

    /// Newly owned metadata and prepared constants; original owners retain their allowances.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}

impl LatticeIndexKind {
    /// Declared thing or index name; its identity remains scoped by the whole question.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::SignedWinding(name)
            | Self::CrystalTranslation(name)
            | Self::QuantizationIndex(name) => name,
            Self::Count(descriptor) => match descriptor.kind() {
                Kind::Count(name, _) => name,
                Kind::Ratio | Kind::Angle | Kind::Named(..) | Kind::Uncertainty(..) => "",
            },
        }
    }
}
