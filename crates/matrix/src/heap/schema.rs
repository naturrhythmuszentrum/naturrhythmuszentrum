// SPDX-License-Identifier: AGPL-3.0-only
//! Immutable numerical shapes contain no physical descriptor or declaration.

use super::Extent;

/// Numerical shape shared independently of value identity and physical interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Schema {
    /// Logical rectangular extent of the complete allocation.
    pub(crate) extent: Extent,
}
