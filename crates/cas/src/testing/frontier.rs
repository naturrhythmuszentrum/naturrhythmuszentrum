// SPDX-License-Identifier: AGPL-3.0-only
//! Synthetic frontier resource tuples and dependency support.
//!
//! Compiled only in the library test binary.

use std::collections::BTreeSet;

use crate::LogicalId;
use crate::frontier::Metrics;
use crate::syntax::VertexKind;

/// Small independently chosen resource tuple.
pub(crate) fn metrics(cost: u64, credits: u64, support: u64) -> Metrics {
    Metrics {
        cost,
        credits,
        peak: 8,
        depth: 2,
        bytes: 10,
        support: BTreeSet::from([LogicalId {
            kind: VertexKind::Proof,
            serial: support,
        }]),
        encoding: vec![1],
    }
}
