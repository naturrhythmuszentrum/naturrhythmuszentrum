// SPDX-License-Identifier: AGPL-3.0-only
//! Complete explicit block tables and immutable joint definitions.

use super::{JointError, JointLimits, Source, SourceFactor, SourceId};
use nrz_scalar::{ArithmeticContext, Enclosure, Finite, Scalar};
use std::rc::Rc;

/// Explicit knowledge of one ordered scalar source block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CovarianceBlock {
    /// Available from the admitted mathematical covariance witness.
    Known,
    /// Explicit zero-covariance assumption, not an independence assertion.
    DeclaredZero(String),
    /// Unavailable to a contraction, even if a completion witness exists.
    Unknown,
}
/// Immutable process-local joint model identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelId(u64);

/// Definition shared by each source; it contains no back-reference to source cells.
#[derive(Clone, Debug)]
pub(crate) struct Definition<P: Scalar> {
    /// Identity shared by exactly repeated admissions.
    id: ModelId,
    /// Exact ordered source identities.
    ids: Vec<SourceId>,
    /// Mathematical PSD witness, independent of displayed covariance rounding.
    factor: SourceFactor<P>,
    /// Complete column-ordered knowledge table.
    blocks: Vec<CovarianceBlock>,
}
/// Ordered declared finite-moment sources with a complete covariance knowledge table.
///
/// Unknown entries never become zeros. The factor supplies a PSD completion; only
/// explicitly available entries may be requested. Rebinding an existing source to
/// a different ordered model refuses conservatively, even for equivalent formulas.
#[derive(Clone, Debug)]
pub struct JointModel<P: Scalar> {
    /// Source declarations, retaining global conflict guards.
    sources: Vec<Source<P>>,
    /// Immutable common covariance relation.
    definition: Rc<Definition<P>>,
}
impl<P: Scalar> JointModel<P> {
    /// Admit a factor-defined joint model with every ordered block explicitly listed.
    ///
    /// # Errors
    /// Refuses incomplete/asymmetric tables, duplicate sources, conflicting global
    /// definitions, unsupported zero declarations, invalid dimensions or resource limits.
    pub fn from_factor(
        sources: &[Source<P>],
        factor: SourceFactor<P>,
        blocks: &[CovarianceBlock],
        limits: JointLimits,
    ) -> Result<Self, JointError> {
        let count = limits.matrix(sources.len(), sources.len())?;
        let work = count
            .checked_mul(factor.columns().max(1))
            .ok_or(JointError::ResourceLimit)?;
        limits.check_work(work)?;
        if factor.rows() != sources.len() || blocks.len() != count {
            return Err(JointError::LengthMismatch);
        }
        check_table(&factor, blocks)?;
        let ids: Vec<_> = sources.iter().map(Source::id).collect();
        for (index, id) in ids.iter().enumerate() {
            if ids[..index].contains(id) {
                return Err(JointError::SourceConflict);
            }
        }
        let proposed = Definition {
            id: ModelId(crate::heap::identity::identity()?),
            ids,
            factor,
            blocks: blocks.to_vec(),
        };
        let definition = shared_definition(sources, proposed)?;
        Ok(Self {
            sources: sources.to_vec(),
            definition,
        })
    }
    /// Joint identity; cloning and exact repeated admission preserve it.
    #[must_use]
    pub fn id(&self) -> ModelId {
        self.definition.id
    }
    /// Immutable model revision; changed definitions require new source identities.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        1
    }
    /// Ordered zero-mean source declarations.
    #[must_use]
    pub fn sources(&self) -> &[Source<P>] {
        &self.sources
    }
    /// Exact represented factor defining the covariance.
    #[must_use]
    pub fn factor(&self) -> &SourceFactor<P> {
        &self.definition.factor
    }
    /// Complete column-ordered knowledge inventory.
    #[must_use]
    pub fn blocks(&self) -> &[CovarianceBlock] {
        &self.definition.blocks
    }
    /// Require every participating block, preserving repetitions and source order.
    ///
    /// # Errors
    /// Unknown blocks, invalid indices and exhausted work refuse before contraction.
    pub fn require_blocks(
        &self,
        rows: &[usize],
        columns: &[usize],
        limits: JointLimits,
    ) -> Result<(), JointError> {
        limits.check_work(
            rows.len()
                .checked_mul(columns.len())
                .ok_or(JointError::ResourceLimit)?,
        )?;
        for &column in columns {
            for &row in rows {
                if matches!(self.block(row, column)?, CovarianceBlock::Unknown) {
                    return Err(JointError::MissingCrossBlock);
                }
            }
        }
        Ok(())
    }
    /// Inspect one block without inferring its value from source identities.
    ///
    /// # Errors
    /// Refuses out-of-range source indices.
    pub fn block(&self, row: usize, column: usize) -> Result<&CovarianceBlock, JointError> {
        let order = self.sources.len();
        if row >= order || column >= order {
            return Err(JointError::LengthMismatch);
        }
        self.definition
            .blocks
            .get(row + order * column)
            .ok_or(JointError::LengthMismatch)
    }
    /// Enclose an available covariance parameter, never a physical deterministic radius.
    ///
    /// # Errors
    /// Refuses unknown blocks, invalid indices, exhausted work and arithmetic failures.
    pub fn covariance(
        &self,
        row: usize,
        column: usize,
        context: &ArithmeticContext,
        limits: JointLimits,
    ) -> Result<Enclosure<P>, JointError> {
        match self.block(row, column)? {
            CovarianceBlock::Unknown => Err(JointError::MissingCrossBlock),
            CovarianceBlock::DeclaredZero(_) => Ok(Enclosure::singleton(Finite::ZERO)),
            CovarianceBlock::Known => self.factor().entry(row, column, context, limits),
        }
    }
}
/// Check symmetry and require a structural proof for each explicitly declared zero.
fn check_table<P: Scalar>(
    factor: &SourceFactor<P>,
    blocks: &[CovarianceBlock],
) -> Result<(), JointError> {
    let order = factor.rows();
    for column in 0..order {
        for row in 0..order {
            let block = &blocks[row + order * column];
            if block != &blocks[column + order * row] {
                return Err(JointError::Asymmetric);
            }
            if let CovarianceBlock::DeclaredZero(reason) = block
                && (reason.trim().is_empty() || !factor.zero(row, column))
            {
                return Err(JointError::InvalidZero);
            }
        }
    }
    Ok(())
}
/// Validate all shared guards before installing any; no partial source reservation escapes.
fn shared_definition<P: Scalar>(
    sources: &[Source<P>],
    proposed: Definition<P>,
) -> Result<Rc<Definition<P>>, JointError> {
    let mut previous = None;
    for source in sources {
        if let Some(old) = source.definition.borrow().as_ref() {
            if old.ids != proposed.ids
                || old.factor != proposed.factor
                || old.blocks != proposed.blocks
            {
                return Err(JointError::SourceConflict);
            }
            previous = Some(Rc::clone(old));
        }
    }
    let definition = previous.unwrap_or_else(|| Rc::new(proposed));
    for source in sources {
        *source.definition.borrow_mut() = Some(Rc::clone(&definition));
    }
    Ok(definition)
}
