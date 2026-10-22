// SPDX-License-Identifier: AGPL-3.0-only
//! Separate base and fibre covers compose complete pieces through existing applications.

mod compose;

use super::invalid;
use crate::budget::Budget;
use crate::error::Fault;
use crate::graph::{Cas, Handle, Reference};
use crate::proof_model::{Statement, VerifiedWitness};
use crate::solving::{FibrePieceData, SolveDraft, SolveReason, SolveStatement, StatementCoverage};

/// One independently complete piece, without a claim about any enclosing cover.
#[derive(Clone, Debug)]
pub struct FamilyPiece {
    /// Exact original family, base stratum, total region, problem and solution set.
    pub descriptor: Handle,
    /// Existing complete coverage theorem on that exact total region.
    pub coverage: VerifiedWitness,
}

/// Fibre cells grouped under one local context over an original base stratum.
#[derive(Clone, Debug)]
pub struct FamilyCoverGroup {
    /// Local context directly below the requested whole-family context.
    pub context: Handle,
    /// Complete local coverage facts, each in this context or a descendant.
    pub pieces: Vec<VerifiedWitness>,
    /// Optional existing cover of this context's admissible total region.
    pub cover: Option<VerifiedWitness>,
}

/// The two independent inventories required for an optional whole-family answer.
#[derive(Clone, Debug)]
pub struct FamilyCoverRequest {
    /// Whole-family context.
    pub context: Handle,
    /// Original unspecialized problem.
    pub problem: Handle,
    /// Optional existing cover of the original Real base.
    pub base_cover: Option<VerifiedWitness>,
    /// Complete pieces grouped by their original base strata.
    pub groups: Vec<FamilyCoverGroup>,
}

/// Missing enclosing evidence never discards an independently complete piece.
#[derive(Clone, Debug)]
pub struct FamilyCoverResult {
    /// All supplied complete pieces, including proved empty pieces.
    pub pieces: Vec<FamilyPiece>,
    /// Existing complete coverage theorem after both covers have replayed.
    pub coverage: Option<VerifiedWitness>,
    /// The precise missing enclosing cover, if any.
    pub pending: Option<SolveReason>,
}

impl Cas {
    /// Compose complete family pieces only after separate base and fibre covers are supplied.
    ///
    /// The exported expansion consists of region construction, context weakening and closure,
    /// and existing total-cover elimination applications.
    ///
    /// # Errors
    /// Refuses changed problems, families, strata, contexts, invalid witnesses or exhausted budgets.
    pub fn family_cover(
        &mut self,
        request: &FamilyCoverRequest,
        generation: &mut Budget,
        checker: &mut Budget,
    ) -> Result<FamilyCoverResult, Fault> {
        self.transaction(generation, |cas, generation| {
            run(cas, request, generation, checker)
        })
    }
}

/// Read only a closed complete piece theorem, never a candidate or conditional answer.
pub(super) fn coverage(witness: &VerifiedWitness) -> Result<&StatementCoverage, Fault> {
    if !witness.is_discharged() {
        return Err(invalid());
    }
    let Statement::Solve(statement) = &witness.judgment().statement else {
        return Err(invalid());
    };
    let SolveStatement::Coverage(value) = statement.as_ref() else {
        return Err(invalid());
    };
    Ok(value)
}

/// Structural piece descriptors retain the original unspecialized problem and exact theorem region.
fn pieces(
    cas: &mut Cas,
    request: &FamilyCoverRequest,
    budget: &mut Budget,
) -> Result<Vec<FamilyPiece>, Fault> {
    let mut result = crate::solving::coefficients::storage::vector(0, budget)?;
    for group in &request.groups {
        for witness in &group.pieces {
            cas.checked(witness.proof())?;
            let claim = coverage(witness)?;
            if claim.problem != request.problem {
                return Err(invalid());
            }
            let over = cas
                .state
                .region(claim.region.logical_id())?
                .lies_over
                .clone()
                .ok_or_else(invalid)?;
            let descriptor = crate::solving::towers::materialize::admit(
                cas,
                SolveDraft::FibrePiece(FibrePieceData {
                    family: Reference::Existing(cas.handle(over.family)),
                    stratum: Reference::Existing(cas.handle(over.stratum)),
                    region: Reference::Existing(claim.region.clone()),
                    problem: Reference::Existing(claim.problem.clone()),
                    set: Reference::Existing(claim.set.clone()),
                }),
                budget,
            )?;
            crate::solving::coefficients::storage::reserve(&mut result, 1, budget)?;
            result.push(FamilyPiece {
                descriptor,
                coverage: witness.clone(),
            });
        }
    }
    Ok(result)
}

/// A missing cover leaves the exact checked piece inventory in the result.
fn run(
    cas: &mut Cas,
    request: &FamilyCoverRequest,
    generation: &mut Budget,
    checker: &mut Budget,
) -> Result<FamilyCoverResult, Fault> {
    let pieces = pieces(cas, request, generation)?;
    let Some(base) = &request.base_cover else {
        return Ok(FamilyCoverResult {
            pieces,
            coverage: None,
            pending: Some(SolveReason::BaseCoveragePending),
        });
    };
    let Some(order) = compose::inventory(cas, request, base, generation)? else {
        return Ok(FamilyCoverResult {
            pieces,
            coverage: None,
            pending: Some(SolveReason::BaseCoveragePending),
        });
    };
    if request.groups.iter().any(|group| group.cover.is_none()) {
        return Ok(FamilyCoverResult {
            pieces,
            coverage: None,
            pending: Some(SolveReason::FibreCoveragePending),
        });
    }
    let coverage = compose::run(cas, request, base, &order, generation, checker)?;
    Ok(FamilyCoverResult {
        pieces,
        coverage: Some(coverage),
        pending: None,
    })
}
