// SPDX-License-Identifier: AGPL-3.0-only
//! One checked presentation closure supplies both exact answer judgments.

use std::collections::BTreeSet;

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, InvalidCode, Location};
use crate::graph::{Cas, LogicalId, size};
use crate::proof_model::{Judgment, Statement, VerifiedWitness};
use crate::solving::{SolveStatement, StatementCoverage, StatementPresentation};

/// Complete-answer matching and dependency traversal share the caller's replay ledger.
const AT: Location = Location::new("complete answer replay");

/// A different theorem cannot supply either half of the expected answer.
const fn mismatch() -> Fault {
    Fault::invalid(InvalidCode::EndpointMismatch, AT)
}

/// Both expectations refer to the same set, region and immutable context.
fn claims<'a>(
    coverage: &'a Judgment,
    presentation: &'a Judgment,
) -> Result<(&'a StatementCoverage, &'a StatementPresentation), Fault> {
    let (Statement::Solve(left), Statement::Solve(right)) =
        (&coverage.statement, &presentation.statement)
    else {
        return Err(mismatch());
    };
    let (SolveStatement::Coverage(left), SolveStatement::Presentation(right)) =
        (left.as_ref(), right.as_ref())
    else {
        return Err(mismatch());
    };
    let coverage_scope = (&coverage.context, coverage.layer, &coverage.space);
    let presentation_scope = (
        &presentation.context,
        presentation.layer,
        &presentation.space,
    );
    let same_scope = coverage_scope == presentation_scope;
    let same_set = left.set == right.set;
    let same_region = left.region == right.region;
    if !(same_scope && same_set && same_region) {
        return Err(mismatch());
    }
    Ok((left, right))
}

/// Only a reachable, freshly checked application with the complete expected judgment qualifies.
fn coverage_dependency(
    cas: &Cas,
    presentation: &VerifiedWitness,
    expected: &[u8],
    budget: &mut Budget,
) -> Result<VerifiedWitness, Fault> {
    let mut visited = BTreeSet::new();
    let mut pending = vec![cas.checked(presentation.proof())?];
    while let Some(proof) = pending.pop() {
        budget.work(1, AT)?;
        if !visited.insert(proof) {
            continue;
        }
        budget.check(BudgetKind::ProofNodes, size(visited.len(), AT)?, AT)?;
        let application = crate::proof_view::application(cas, proof, budget)?;
        if claims(&application.judgment, presentation.judgment()).is_ok()
            && crate::wire_build::judgment_key(cas, &application.judgment, budget)? == expected
        {
            return cas.seal_scoped(&cas.handle(proof), budget);
        }
        append(&mut pending, &application.premises, budget)?;
    }
    Err(mismatch())
}

/// Bound the worklist before copying checked premise references.
fn append(
    pending: &mut Vec<LogicalId>,
    premises: &[LogicalId],
    budget: &mut Budget,
) -> Result<(), Fault> {
    let count = pending
        .len()
        .checked_add(premises.len())
        .ok_or_else(mismatch)?;
    budget.check(BudgetKind::Edges, size(count, AT)?, AT)?;
    budget.work(size(premises.len(), AT)?, AT)?;
    pending
        .try_reserve(premises.len())
        .map_err(|_| Fault::allocation(AT))?;
    pending.extend_from_slice(premises);
    Ok(())
}

impl Cas {
    /// Replay one presentation witness and return its exact checked coverage and presentation.
    ///
    /// Both expected judgments must name the same set, region and context in this store.
    /// Every application in the portable closure is checked once, even in a populated receiver.
    /// The coverage must occur in that closure; presentation alone never implies completeness.
    /// The returned tuple is `(coverage, presentation)`, each independently exportable.
    /// All checking, comparison and sealing consume the same cumulative `budget`.
    /// Failure publishes neither imported theorem and never refunds consumed work.
    ///
    /// # Errors
    /// Refuses malformed witnesses, mismatched expectations, missing coverage dependencies,
    /// false applications and exhausted resources.
    pub fn replay_answer(
        &mut self,
        bytes: &[u8],
        expected_coverage: &Judgment,
        expected_presentation: &Judgment,
        budget: &mut Budget,
    ) -> Result<(VerifiedWitness, VerifiedWitness), Fault> {
        crate::wire_decode::magic(bytes)?;
        self.proof_limits(budget, |cas, budget| {
            crate::proof_check::replay_budget(budget, |budget| {
                claims(expected_coverage, expected_presentation)?;
                crate::proof_view::validate_judgment(cas, expected_coverage, budget)?;
                crate::proof_view::validate_judgment(cas, expected_presentation, budget)?;
                let coverage_key = crate::wire_build::judgment_key(cas, expected_coverage, budget)?;
                let presentation_key =
                    crate::wire_build::judgment_key(cas, expected_presentation, budget)?;
                cas.transaction(budget, |candidate, budget| {
                    let presentation = crate::wire_decode::inspect(candidate, bytes, budget)?;
                    let actual = crate::wire_build::judgment_key(
                        candidate,
                        presentation.judgment(),
                        budget,
                    )?;
                    if actual != presentation_key {
                        return Err(mismatch());
                    }
                    let coverage =
                        coverage_dependency(candidate, &presentation, &coverage_key, budget)?;
                    Ok((coverage, presentation))
                })
            })
        })
    }
}
