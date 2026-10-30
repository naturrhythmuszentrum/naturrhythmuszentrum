// SPDX-License-Identifier: AGPL-3.0-only
//! Finite logarithm evidence; producer and replay have separate recurrence traversals.

mod check;
mod produce;
mod sizing;
mod wire;

pub(super) use check::check;
pub(super) use produce::produce;
pub(super) use sizing::{Plan, plan};
pub(super) use wire::{decode, encode};

use super::{AT, Interval};
use crate::{Fault, Rational};

/// Proposed finite atanh recurrence, including its outward arithmetic error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogSeries {
    /// Number of included odd terms; zero is reserved for an exact skipped zero.
    pub terms: u32,
    /// Enclosure of the included sum.
    pub sum: Interval,
    /// Enclosure of the first omitted odd power.
    pub power: Interval,
    /// Enclosure of the geometric remainder expression.
    pub tail: Interval,
}

/// Proposed reduction and finite series for one positive rational endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEndpoint {
    /// Signed binary reduction exponent.
    pub exponent: i64,
    /// Reduced argument in the half-open interval from one to two.
    pub mantissa: Rational,
    /// Exact reduced atanh argument.
    pub argument: Rational,
    /// Finite series for the mantissa; exact zero skips it.
    pub reduced: LogSeries,
    /// Finite series at one third; a zero exponent skips it.
    pub two: LogSeries,
}

/// Logarithm evidence joined to one original postorder record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEvidence {
    /// Index of the original logarithm node's bound in the transcript.
    pub record: u32,
    /// Evidence for its lower argument endpoint.
    pub lower: LogEndpoint,
    /// Evidence for its upper argument endpoint.
    pub upper: LogEndpoint,
}

/// An unseparated computed domain requests another round, never an implicit clamp.
pub(super) const UNSEPARATED: crate::Location =
    crate::Location::new("logarithm domain unseparated");

/// Logical bytes of all finite evidence, accounted before copying or retention.
pub(super) fn bytes(records: &[LogEvidence]) -> u64 {
    records.iter().fold(0_u64, |total, record| {
        total
            .saturating_add(64)
            .saturating_add(endpoint_bytes(&record.lower))
            .saturating_add(endpoint_bytes(&record.upper))
    })
}

/// Account for a finite endpoint before equality checks and bounded copies.
pub(super) fn endpoint_bytes(endpoint: &LogEndpoint) -> u64 {
    [&endpoint.reduced, &endpoint.two].into_iter().fold(
        super::number_bytes(&endpoint.mantissa)
            .saturating_add(super::number_bytes(&endpoint.argument))
            .saturating_add(64),
        |size, series| {
            size.saturating_add(32)
                .saturating_add(super::interval_bytes(&series.sum))
                .saturating_add(super::interval_bytes(&series.power))
                .saturating_add(super::interval_bytes(&series.tail))
        },
    )
}

/// Malformed finite evidence has no mathematical standing.
pub(super) const fn invalid() -> Fault {
    Fault::invalid(crate::InvalidCode::EndpointMismatch, AT)
}
