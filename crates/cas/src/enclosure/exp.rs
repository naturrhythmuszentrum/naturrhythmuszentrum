// SPDX-License-Identifier: AGPL-3.0-only
//! Finite exponential proposals and independent exact replay.

mod check;
mod produce;
pub(super) mod sizing;
mod wire;

pub(super) use check::{check, endpoint as check_endpoint};
pub(super) use produce::{endpoint as produce_endpoint, produce};
pub(super) use sizing::plan;
pub(super) use wire::{decode, encode};

use super::{AT, Interval};
use crate::{Fault, Rational};

/// Finite Taylor recurrence and its geometric remainder enclosure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpSeries {
    /// Highest included degree, so the sum has this count plus one terms.
    pub degree: u32,
    /// Enclosure of the partial sum.
    pub sum: Interval,
    /// Enclosure of the first omitted term, including its factorial divisor.
    pub omitted: Interval,
    /// Enclosure of the geometric tail expression.
    pub tail: Interval,
}

/// Proposed nonnegative reduction of one original signed argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpEndpoint {
    /// Number of interval squarings after reduction.
    pub squarings: u32,
    /// Nonnegative reduced argument, at most one half.
    pub argument: Rational,
    /// Finite recurrence; degree zero denotes only the exact zero shortcut.
    pub series: ExpSeries,
}

/// Monotone exponential evidence joined to an original postorder record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpEvidence {
    /// Index of the exponential node's proposed bound.
    pub record: u32,
    /// Lower argument endpoint evidence.
    pub lower: ExpEndpoint,
    /// Upper argument endpoint evidence.
    pub upper: ExpEndpoint,
}

/// Bounded logical size of all finite recurrence summaries.
pub(super) fn bytes(records: &[ExpEvidence]) -> u64 {
    records.iter().fold(0_u64, |size, record| {
        size.saturating_add(64)
            .saturating_add(endpoint_bytes(&record.lower))
            .saturating_add(endpoint_bytes(&record.upper))
    })
}

/// Account before copying or comparing supplied endpoint evidence.
pub(super) fn endpoint_bytes(endpoint: &ExpEndpoint) -> u64 {
    [
        &endpoint.series.sum,
        &endpoint.series.omitted,
        &endpoint.series.tail,
    ]
    .into_iter()
    .fold(
        super::number_bytes(&endpoint.argument).saturating_add(64),
        |size, value| size.saturating_add(super::interval_bytes(value)),
    )
}

/// Malformed analytic evidence is a refusal, never a mathematical refutation.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(crate::InvalidCode::EndpointMismatch, AT)
}
