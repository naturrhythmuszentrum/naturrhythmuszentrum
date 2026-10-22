// SPDX-License-Identifier: AGPL-3.0-only
//! Bindings reconstructs expected judgments without trusting a witness's inspected claim.

use super::{ReadingError, ReadingProjection, meaning, support};
use nrz_cas::{
    Cas,
    domain::Layer,
    limits::Budget,
    proof::{DomainContract, Judgment, Statement, VerifiedWitness},
};

/// Checked equality between two exact physical projections on their entire family.
#[derive(Clone, Debug)]
pub struct ReadingRelation {
    /// First physical endpoint and its exact selection.
    left: ReadingProjection,
    /// Second physical endpoint and its exact selection.
    right: ReadingProjection,
    /// Discharged CAS equality under the retained family context.
    witness: VerifiedWitness,
}

impl ReadingRelation {
    /// Prove compatibility after checking both complete original physical declarations.
    /// # Errors
    /// Refuses unrelated families, physical mismatches, contradictory maps and exhausted limits.
    pub fn prove(
        cas: &mut Cas,
        left: ReadingProjection,
        right: ReadingProjection,
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let _expected = expected(cas, &left, &right, budget)?;
        let witness = cas.prove_reading_equality(
            left.reading.family.context(),
            left.node(),
            right.node(),
            budget,
        )?;
        Ok(Self {
            left,
            right,
            witness,
        })
    }

    /// Replay portable CAS evidence against independently reconstructed physical endpoints.
    ///
    /// Physical declarations are independently checked in the receiving graph. The
    /// bytes supply only the mathematical proof, never units or live-storage authority.
    /// # Errors
    /// Refuses changed selections, definitions, declarations, domains or physical meanings.
    pub fn replay(
        cas: &mut Cas,
        left: ReadingProjection,
        right: ReadingProjection,
        bytes: &[u8],
        budget: &mut Budget,
    ) -> Result<Self, ReadingError> {
        let expected = expected(cas, &left, &right, budget)?;
        let witness = cas.replay(bytes, &expected, budget)?;
        Ok(Self {
            left,
            right,
            witness,
        })
    }

    /// Export the complete mathematical definition and proof closure.
    /// # Errors
    /// Refuses foreign stores or exhausted cumulative export limits.
    pub fn export(&self, cas: &Cas, budget: &mut Budget) -> Result<Vec<u8>, ReadingError> {
        Ok(cas.export_witness(&self.witness, budget)?)
    }
    /// Exact left projection, including selected-copy identity.
    #[must_use]
    pub const fn left(&self) -> &ReadingProjection {
        &self.left
    }
    /// Exact right projection, including selected-copy identity.
    #[must_use]
    pub const fn right(&self) -> &ReadingProjection {
        &self.right
    }
    /// Independently checked whole-family equality and definedness witness.
    #[must_use]
    pub const fn witness(&self) -> &VerifiedWitness {
        &self.witness
    }

    /// Match the exact ordered claim before consuming the evidence in an adapter.
    /// # Errors
    /// Refuses substituted definitions, projections, kinds, references or origins.
    pub fn check_targets(
        &self,
        left: &ReadingProjection,
        right: &ReadingProjection,
    ) -> Result<(), ReadingError> {
        if same(&self.left, left) && same(&self.right, right) {
            Ok(())
        } else {
            Err(ReadingError::Relation)
        }
    }
}

/// Compare complete local endpoint identity before permitting witness reuse.
fn same(left: &ReadingProjection, right: &ReadingProjection) -> bool {
    let node = left.node == right.node;
    let reading = left.reading.node == right.reading.node;
    let coordinates = left.coordinates == right.coordinates;
    let entries = left.entries == right.entries;
    node && reading && coordinates && entries
}

/// Reconstruct the full scope, domain and equality from independently checked endpoints.
fn expected(
    cas: &Cas,
    left: &ReadingProjection,
    right: &ReadingProjection,
    budget: &mut Budget,
) -> Result<Judgment, ReadingError> {
    budget.work(1, support::AT)?;
    let left_family = left.reading.family();
    let right_family = right.reading.family();
    if left_family.root() != right_family.root() || left_family.context() != right_family.context()
    {
        return Err(ReadingError::Unrelated);
    }
    meaning::validate(left_family.entries(), budget)?;
    meaning::validate(right_family.entries(), budget)?;
    if left_family.root_entries != right_family.root_entries {
        return Err(ReadingError::Meaning);
    }
    if left.entries.len() != right.entries.len() {
        return Err(ReadingError::Shape);
    }
    for (first, second) in left.entries.iter().zip(&right.entries) {
        first.check(second)?;
    }
    Ok(Judgment {
        context: left_family.context().clone(),
        layer: Layer::Pointwise,
        space: None,
        statement: Statement::Equality {
            left: left.node.clone(),
            right: right.node.clone(),
            domain: cas.admissible_region(left_family.context())?,
            domain_contract: DomainContract::DefinedAndEqual,
        },
        residuals: Vec::new(),
    })
}
