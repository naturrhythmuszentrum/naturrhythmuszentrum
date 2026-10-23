// SPDX-License-Identifier: AGPL-3.0-only
//! A closed dimensionless coefficient recipe retains its original exact operator correspondence.

mod build;
mod guard;
mod jacobian;
mod storage;

use super::{CoordinateNormalization, ReadingError, support};
use nrz_cas::{
    Cas, Handle,
    limits::Budget,
    proof::VerifiedWitness,
    readings::{AffineMap, ExactAffineMap, ReuseKey},
};
use std::rc::Rc;

pub use jacobian::{
    ReciprocalJacobian, ReciprocalJacobianCache, ReciprocalJacobianKey, SourceJacobianUsage,
};

/// Proposed coefficient array formed from positive normalized reciprocals.
#[derive(Clone, Debug)]
pub struct ReciprocalMapDefinition {
    /// Ordered formal Real scalar inputs, matched to the physical reference entries.
    pub inputs: Vec<Handle>,
    /// Result operator rows and columns; entries follow logical column order.
    pub shape: [u32; 2],
    /// Exact affine map from `reference[i]/input[i]` to the flattened result coefficients.
    pub map: AffineMap,
    /// Original operator whose whole-family equality must be checked before execution.
    pub original: ExactAffineMap,
}

/// Checked closed recipe for a dimensionless operator over independently typed positive inputs.
#[derive(Clone, Debug)]
pub struct PositiveReciprocalMap(Rc<Definition>);

/// One immutable declaration and complete correspondence, shared by all numerical executions.
#[derive(Debug)]
struct Definition {
    /// Original ordered positive difference references and their checked inverse.
    frame: CoordinateNormalization,
    /// Every formal input and exact affine coefficient definition.
    proposed: ReciprocalMapDefinition,
    /// Exact reciprocal expressions, including operands unused by the affine map.
    reciprocals: Vec<Handle>,
    /// Explicit numerical law as a native exact coefficient matrix.
    image: ExactAffineMap,
    /// Whole-family correspondence to the original ordered operator.
    witness: VerifiedWitness,
    /// Authenticated original expression, context and correspondence identity.
    key: ReuseKey,
    /// Conservative complete retained native metadata and certificate charge.
    bytes: u64,
}

impl PositiveReciprocalMap {
    /// Check every physical input before proposing and proving the complete operator image.
    ///
    /// Distinct formal input slots are coherent scalar differences. Execution must bind each slot
    /// to that physical meaning and prove its entire numerical enclosure strictly positive,
    /// including unused slots. Original syntax admits total polynomial/matrix arithmetic and
    /// division by formal inputs or nonzero rational constants, preserving even cancelled
    /// denominator obligations. The result entries are dimensionless coefficients.
    /// # Errors
    /// Refuses points, mismatched shapes/declarations, changed original operators,
    /// unavailable whole-family correspondence and cumulative exact resource limits.
    pub fn prove(
        cas: &mut Cas,
        frame: &CoordinateNormalization,
        proposed: ReciprocalMapDefinition,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let retained = storage::proposal(frame, &proposed, budget)?;
        let (reciprocals, image) =
            budget.with_retained_payload(retained, support::AT, |budget| {
                build::check(cas, frame, &proposed, budget)?;
                guard::check(cas, frame, &proposed, budget)?;
                build::image(cas, frame, &proposed, budget)
            })?;
        let witness = budget.with_retained_payload(retained, support::AT, |budget| {
            cas.prove_reading_equality(
                frame.context(),
                proposed.original.expression(),
                image.expression(),
                budget,
            )
        })?;
        if !witness.is_discharged() {
            return Err(ReadingError::Definition);
        }
        let bytes = storage::witness(cas, &witness, retained, budget)?;
        let key = budget.with_retained_payload(bytes, support::AT, |budget| {
            cas.witness_reuse_key(&witness, budget)
        })?;
        let bytes = storage::result(bytes, &key, budget)?;
        Ok(Self(Rc::new(Definition {
            frame: frame.clone(),
            proposed,
            reciprocals,
            image,
            witness,
            key,
            bytes,
        })))
    }

    /// Original physical references in formal input order; numerical values never select them.
    #[must_use]
    pub fn frame(&self) -> &CoordinateNormalization {
        &self.0.frame
    }
    /// Ordered exact formal input declarations.
    #[must_use]
    pub fn inputs(&self) -> &[Handle] {
        &self.0.proposed.inputs
    }
    /// Exact normalized reciprocal temporary definitions, one per formal input.
    #[must_use]
    pub fn reciprocals(&self) -> &[Handle] {
        &self.0.reciprocals
    }
    /// Operator rows and columns, with column-ordered output coefficients.
    #[must_use]
    pub fn shape(&self) -> [u32; 2] {
        self.0.proposed.shape
    }
    /// Complete exact affine coefficient map, including signs, mixing and offsets.
    #[must_use]
    pub fn map(&self) -> &AffineMap {
        &self.0.proposed.map
    }
    /// Original ordered operator expression before recipe coefficient reduction.
    #[must_use]
    pub fn original(&self) -> &ExactAffineMap {
        &self.0.proposed.original
    }
    /// Native exact operator image computed by the closed numerical recipe.
    #[must_use]
    pub fn image(&self) -> &ExactAffineMap {
        &self.0.image
    }
    /// Complete correspondence, with its original domain and hypotheses retained.
    #[must_use]
    pub fn witness(&self) -> &VerifiedWitness {
        &self.0.witness
    }
    /// Complete authenticated exact identity; numerical versions and arithmetic remain separate.
    #[must_use]
    pub fn reuse_key(&self) -> &ReuseKey {
        &self.0.key
    }
    /// Retained payload reserved by nested numerical lowering operations.
    #[must_use]
    pub fn retained_payload_bytes(&self) -> u64 {
        self.0.bytes
    }
}
