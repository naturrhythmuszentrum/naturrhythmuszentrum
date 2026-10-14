// SPDX-License-Identifier: AGPL-3.0-only
//! Explicit untrusted data for the five closed native case schemas.

use crate::graph::Handle;
use crate::region::RegionCertificate;

/// Typed association, restriction and exact-cell evidence for a case application.
///
/// Every rule rejects fields it does not consume. Ordered construction uses the reached
/// regions and source-order arms. Domain checking uses defined pieces. Selection uses
/// its selected region and arm. Congruence uses complete endpoint association inventories. Elimination
/// uses cover members and their exact local frames. Cells are always independently rebuilt.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CaseCertificate {
    /// Ordered reached regions, defined pieces, selection region or cover members.
    pub regions: Vec<Handle>,
    /// Ordered source associations, selected arm or canonical endpoint association inventories.
    pub arms: Vec<Handle>,
    /// Exact local frames, in cover-member order, for elimination only.
    pub frames: Vec<Handle>,
    /// Exact rational partitions consumed in the rule's prescribed order.
    pub cells: Vec<RegionCertificate>,
}
