// SPDX-License-Identifier: AGPL-3.0-only
//! One checked presentation closure supplies both exact answer judgments.
//!
//! # Complete-answer replay and acceptance
//!
//! `Cas::replay_answer(bytes, expected_coverage, expected_presentation, budget)` accepts one ordinary presentation witness and returns `(coverage, presentation)` as two sealed, separately exportable witnesses. Both expected judgments name the same set, region and context. The method checks the entire presentation closure once, compares its complete expected judgment, then finds a reachable checked Coverage application with the complete expected coverage judgment. It seals that dependency without rerunning its rules. A valid presentation without that exact coverage dependency is refused. Both outputs publish atomically; failed comparison, decoding or exhaustion rolls back the import and retains spent work.
//!
//! The caller reconstructs the original problem, context, region and requested form independently; an admitted proposed solution set supplies syntax only. Canonical judgment comparison retains the entire theorem, semantic header including branch policy, layer, space and residual tuple set. Checked applications retain their original requirements and qualified hypotheses. Reuse is confined to identical checked proof identities within this one import under one fixed registry, arithmetic implementation and active budget policy. No producer cache, previous replay or cross-revision cache supplies authority. Logical search costs remain separate from mathematical witnesses. The current `NRZCAS04` format, primitive versions, logical v10 weights and `Limits::blind_cubic()` ceilings are unchanged.
//!
//! The crate acceptance test starts each receiver with independently reconstructed source and only the proposed set's admitted syntax, asserting zero checked proofs before replay. It covers all six symbolic boundaries, including renamed parameters, checks independent factor/sign oracles, compares every primitive's coefficient and Work delta with standalone presentation, and requires canonical re-export of both returned witnesses. Public tests cover changed source, region, form, set, branch policy and premise, missing or different-set coverage, rollback and prior ledger charges. Per-application before/after ledgers are available only in the test build (´test:crate:complete-answer-symbolic-fixtures´), (´test:integration:complete-answer-portable-pair´), (´test:integration:complete-answer-expected-mutations´), (´test:integration:complete-answer-policy-mutation´), (´test:integration:complete-answer-premise-mutation´), (´test:integration:complete-answer-requires-coverage-dependency´), (´test:integration:complete-answer-rejects-different-covered-set´), (´test:integration:complete-answer-cumulative-ledger´).

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
