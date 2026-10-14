// SPDX-License-Identifier: AGPL-3.0-only
//! Logical expression identities for graph-independent frozen-search fixtures.
//!
//! Compiled only in the library test binary.

use crate::LogicalId;
use crate::syntax::VertexKind;

/// A logical expression identifier in a graph-independent fixture.
pub(crate) const fn node(serial: u64) -> LogicalId {
    LogicalId {
        kind: VertexKind::Expression,
        serial,
    }
}
