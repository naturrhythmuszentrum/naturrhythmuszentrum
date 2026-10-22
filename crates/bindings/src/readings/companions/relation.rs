// SPDX-License-Identifier: AGPL-3.0-only
//! Companion equality requires both physical target proofs and exact parameter lineage.

use super::super::{ReadingRelation, support};
use super::{CompanionReading, CompanionReadingError};
use nrz_cas::{
    Cas,
    domain::Layer,
    limits::Budget,
    proof::{DomainContract, Judgment, Statement, VerifiedWitness},
};

/// A complete-family companion equation with its checked physical target equations.
#[derive(Clone, Debug)]
pub struct CompanionRelation {
    /// Exact ordered companion endpoints.
    endpoints: [CompanionReading; 2],
    /// Physical equality for each ordered output target.
    targets: Vec<ReadingRelation>,
    /// Checked equality of the complete parameter reading maps.
    witness: VerifiedWitness,
}

impl CompanionRelation {
    /// Prove compatible companion readings without making repeated sources independent.
    /// # Errors
    /// Refuses changed kinds, target equations, source parameters or contradictory maps.
    pub fn prove(
        cas: &mut Cas,
        endpoints: [CompanionReading; 2],
        targets: Vec<ReadingRelation>,
        budget: &mut Budget,
    ) -> Result<Self, CompanionReadingError> {
        let claim = expected(cas, &endpoints, &targets, budget)?;
        let witness = cas.prove_reading_equality(
            &claim.context,
            endpoints[0].node(),
            endpoints[1].node(),
            budget,
        )?;
        Ok(Self {
            endpoints,
            targets,
            witness,
        })
    }
    /// Reconstruct expected physical targets and parameter equality before fresh CAS replay.
    /// # Errors
    /// Refuses substituted selections, sources, target equations, domains or proof bytes.
    pub fn replay(
        cas: &mut Cas,
        endpoints: [CompanionReading; 2],
        targets: Vec<ReadingRelation>,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Self, CompanionReadingError> {
        let claim = expected(cas, &endpoints, &targets, budget)?;
        let witness = cas.replay(bytes, &claim, budget)?;
        Ok(Self {
            endpoints,
            targets,
            witness,
        })
    }
    /// Exact companion definitions related by the proof.
    #[must_use]
    pub const fn endpoints(&self) -> &[CompanionReading; 2] {
        &self.endpoints
    }
    /// Both covariance target proofs, or the sole bound target proof.
    #[must_use]
    pub fn targets(&self) -> &[ReadingRelation] {
        &self.targets
    }
    /// Complete-family parameter equation, independent of numerical payloads.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }
}

/// Reconstruct the full expected equation from checked physical declarations.
fn expected(
    cas: &Cas,
    endpoints: &[CompanionReading; 2],
    relations: &[ReadingRelation],
    budget: &mut Budget,
) -> Result<Judgment, CompanionReadingError> {
    support::capacity::<ReadingRelation>(relations.len(), budget)?;
    let [left, right] = endpoints;
    if left.kind() != right.kind() {
        return Err(CompanionReadingError::Kind);
    }
    if relations.len() != left.targets.len() || relations.len() != right.targets.len() {
        return Err(CompanionReadingError::Target);
    }
    for ((first, second), relation) in left.targets.iter().zip(&right.targets).zip(relations) {
        let left_matches = relation.left().reading().node() == first.node();
        let right_matches = relation.right().reading().node() == second.node();
        let whole_left = relation
            .left()
            .coordinates()
            .iter()
            .copied()
            .eq(0..u32::try_from(first.entries().len())
                .map_err(|_| CompanionReadingError::Target)?);
        let whole_right = relation
            .right()
            .coordinates()
            .iter()
            .copied()
            .eq(0..u32::try_from(second.entries().len())
                .map_err(|_| CompanionReadingError::Target)?);
        let value_space = relation.left().space() == super::super::ProjectionSpace::Value
            && relation.right().space() == super::super::ProjectionSpace::Value;
        if !left_matches || !right_matches || !whole_left || !whole_right || !value_space {
            return Err(CompanionReadingError::Target);
        }
    }
    if left.parameters.context != right.parameters.context || left.units != right.units {
        return Err(CompanionReadingError::Target);
    }
    let context = left.parameters.context.clone();
    Ok(Judgment {
        context: context.clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: left.node().clone(),
            right: right.node().clone(),
            domain: cas.admissible_region(&context)?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    })
}
