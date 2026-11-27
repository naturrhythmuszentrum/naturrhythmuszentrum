// SPDX-License-Identifier: AGPL-3.0-only
//! Independent timestamp intervals retain their original equations beside any relaxation.

mod admit;
mod exact;
mod fibre;
mod outer;
mod shared;

pub use exact::LcTimingCover;
pub use outer::LcTimingOuterCover;
pub use shared::{LcSharedTiming, LcSharedTimingCover, LcSharedTimingWitness};

use super::{AT, LcRelation, LcRelationError, storage};
use crate::Budget;
use nrz_cas::Rational;
use std::rc::Rc;

/// Finite rational deterministic support, with independently checked endpoint order.
type Bounds = [Rational; 2];

/// Independent sample timing supports `t_i=nominal_i+delta_i`, without a shared error.
#[derive(Clone, Debug)]
pub struct LcSampleTimes(Rc<Question>);

/// Original intervals and their nominal coordinates survive every derived projection.
#[derive(Debug)]
struct Question {
    /// Complete original physical question and common epoch.
    relation: LcRelation,
    /// Explicit nominal coordinates, not inferred physical witnesses.
    nominal: Vec<Rational>,
    /// Intersections of every original timestamp declaration.
    times: Vec<Bounds>,
    /// A separate error support for each observation, in the same order.
    errors: Vec<Bounds>,
    /// Retained positive rate support used by the conservative multiplication.
    rate: Bounds,
    /// Conservative new exact ownership, excluding the original relation.
    bytes: u64,
}

impl LcSampleTimes {
    /// Retain finite rational timestamp boxes relative to explicit nominal coordinates.
    ///
    /// Repeated time and rate declarations intersect. The nominal values need not be
    /// feasible or lie inside the boxes; they only define each independent error coordinate.
    /// # Errors
    /// Refuses missing or unsupported finite supports, nonpositive rates and exhausted Work.
    pub fn new(
        relation: LcRelation,
        nominal: Vec<Rational>,
        budget: &mut Budget,
    ) -> Result<Self, LcRelationError> {
        let (times, rate) = admit::question(&relation, &nominal, budget)?;
        let mut errors = Vec::with_capacity(times.len());
        for (time, center) in times.iter().zip(&nominal) {
            budget.work(1, AT)?;
            errors.push([time[0].sub(center, budget)?, time[1].sub(center, budget)?]);
        }
        let bytes = nominal
            .iter()
            .chain(times.iter().flatten())
            .chain(errors.iter().flatten())
            .chain(&rate)
            .fold(2_048_u64, |sum, value| {
                sum.saturating_add(storage::rational(value))
            });
        storage::retained(
            bytes.saturating_add(relation.retained_payload_bytes()),
            budget,
        )?;
        Ok(Self(Rc::new(Question {
            relation,
            nominal,
            times,
            errors,
            rate,
            bytes,
        })))
    }

    /// Original relation, including every repeated input and the named common epoch.
    #[must_use]
    pub fn relation(&self) -> &LcRelation {
        &self.0.relation
    }

    /// Explicit nominal coordinates used to define independent timing errors.
    #[must_use]
    pub fn nominal_times(&self) -> &[Rational] {
        &self.0.nominal
    }

    /// Original per-sample timestamp intersections, never a shared offset domain.
    #[must_use]
    pub fn time_boxes(&self) -> &[Bounds] {
        &self.0.times
    }

    /// Each sample's own error interval, preserving unequal and asymmetric errors.
    #[must_use]
    pub fn errors(&self) -> &[Bounds] {
        &self.0.errors
    }

    /// Original positive finite rate support, before projection or scale elimination.
    #[must_use]
    pub fn rate_support(&self) -> &Bounds {
        &self.0.rate
    }

    /// Identity of the original independent-timing question and nominal coordinates.
    #[must_use]
    pub fn shares_allocation(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Conservative new exact ownership, excluding the original relation.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}
