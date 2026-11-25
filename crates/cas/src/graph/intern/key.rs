// SPDX-License-Identifier: AGPL-3.0-only
//! Structural keys may retain an immutable signature without copying its bytes.

use crate::budget::{Budget, BudgetKind};
use crate::domain_model::Header;
use crate::encoding::Encoder;
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{VertexKind, encode_id, size};
use crate::header::FrozenEnvironment;

/// Ordinary canonical tag and absent-parent marker for a frozen core header.
const HEADER_PREFIX: [u8; 2] = [VertexKind::Header.tag(), 0];

/// Index and journal ownership of exactly the ordinary canonical key sequence.
#[derive(Clone, Debug)]
pub(crate) enum StoredKey {
    /// Existing contiguous encoding for ordinary graph records.
    Plain(Vec<u8>),
    /// Immutable complete key shared by a judgment, index and rollback record.
    Shared(std::sync::Arc<[u8]>),
    /// Fresh header framing around its existing immutable signature allocation.
    Header {
        /// The same checked owner retained by the graph's header payload.
        environment: FrozenEnvironment,
        /// Actual scalar-space identities and the empty declaration-space inventory.
        suffix: Vec<u8>,
    },
}

impl StoredKey {
    /// Construct fresh framing after the caller authenticates the frozen environment.
    pub(crate) fn header(header: &Header, budget: &mut Budget) -> Result<Self, Fault> {
        let at = Location::new("structural key");
        if header.parent.is_some() || !header.spaces.is_empty() {
            return Err(Fault::invalid(InvalidCode::Context, at));
        }
        let length = header
            .scalar_spaces
            .len()
            .checked_mul(9)
            .and_then(|length| length.checked_add(10))
            .and_then(|length| length.checked_add(header.environment.canonical_bytes().len()))
            .ok_or_else(|| Fault::capacity(BudgetKind::PayloadBytes, at))?;
        for kind in [BudgetKind::PayloadBytes, BudgetKind::StagedPayloadBytes] {
            budget.check(kind, size(length, at)?, at)?;
        }
        // Two fixed framing bytes and one acquisition of the shared signature owner.
        budget.work(3, at)?;
        let mut out = Encoder::new(budget, at);
        out.count(header.scalar_spaces.len())?;
        for space in &header.scalar_spaces {
            encode_id(*space, &mut out)?;
        }
        out.count(0)?;
        Ok(Self::Header {
            environment: header.environment.clone(),
            suffix: out.finish(),
        })
    }

    /// Borrow the full byte sequence in order, without a temporary concatenation.
    pub(crate) fn parts(&self) -> [&[u8]; 3] {
        match self {
            Self::Plain(bytes) => [bytes, &[], &[]],
            Self::Shared(bytes) => [bytes, &[], &[]],
            Self::Header {
                environment,
                suffix,
            } => [&HEADER_PREFIX, environment.canonical_bytes(), suffix],
        }
    }

    /// Complete canonical length retains the ordinary probe and containment allowances.
    pub(crate) fn len(&self) -> usize {
        self.parts()
            .into_iter()
            .fold(0_usize, |length, part| length.saturating_add(part.len()))
    }
}

impl From<Vec<u8>> for StoredKey {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Plain(bytes)
    }
}

impl From<std::sync::Arc<[u8]>> for StoredKey {
    fn from(bytes: std::sync::Arc<[u8]>) -> Self {
        Self::Shared(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::StoredKey;
    use crate::error::Location;
    use crate::graph::intern::{InternIndex, StoreToken};
    use crate::graph::{LogicalId, VertexKind};
    use crate::testing::common::{TestResult, default_budget};
    use std::sync::Arc;

    /// Large immutable bytes have one owner allocation across inventory, index and journal.
    /// # Panics
    /// Panics if index or journal retention copies the key rather than its owner.
    /// ´claim:cas:prepared-expm1-shared-key´
    /// ´test:unit:prepared-expm1-shared-key´
    #[test]
    fn prepared_expm1_shared_key() -> TestResult {
        let inventory: Arc<[u8]> = vec![5; 65_536].into();
        let mut index = InternIndex::default();
        let id = LogicalId {
            kind: VertexKind::AnonymousLemma,
            serial: 0,
        };
        let stored: StoredKey = Arc::clone(&inventory).into();
        let at = Location::new("prepared expm1 shared key test");
        let (position, _) = index.prepare_insert_key(&stored, false, &mut default_budget()?, at)?;
        index.insert_prepared_key(position, stored, id, &Arc::new(StoreToken::default()));
        let journal: StoredKey = Arc::clone(&inventory).into();
        assert_eq!(Arc::strong_count(&inventory), 3);
        assert_eq!(journal.parts()[0].as_ptr(), inventory.as_ptr());
        assert_eq!(
            index.get(&inventory, 0, &mut default_budget()?, at)?,
            Some(&id)
        );
        drop(journal);
        assert_eq!(Arc::strong_count(&inventory), 2);
        Ok(())
    }
}
