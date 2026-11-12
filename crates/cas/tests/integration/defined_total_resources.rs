// SPDX-License-Identifier: AGPL-3.0-only
//! Whole-call resource boundaries preserve spent Work and roll back publication.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::defined_total::Fixture;
use crate::testing::defined_total_minimal as inventory;
use crate::testing::half_pi_limits::ceiling;
use crate::testing::series_residual::admit;
use nrz_cas::domain::{AmbientDomain, ContextDraft, RegionDraft, RegionExpr};
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::proof::{ApplicationDraft, Judgment, VerifiedWitness};
use nrz_cas::syntax::{AtomicSort, Branch, Draft, Field, Operator};
use nrz_cas::{Cas, Exhaustion, Fault, FaultKind, Handle, Location, Reference};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Produce,
    Check,
    Export,
    Replay,
}

enum Action {
    Produce(Handle, Handle),
    Check(Handle, ApplicationDraft),
    Export(VerifiedWitness),
    Replay(Vec<u8>, Judgment),
}

impl Action {
    fn run(&self, cas: &mut Cas, budget: &mut Budget) -> Result<(), Fault> {
        match self {
            Self::Produce(function, context) => {
                cas.defined_total(function, context, budget).map(|_| ())
            }
            Self::Check(context, draft) => cas
                .check_proof(context, std::slice::from_ref(draft), budget)
                .map(|_| ()),
            Self::Export(witness) => cas.export_witness(witness, budget).map(|_| ()),
            Self::Replay(bytes, expected) => cas.replay(bytes, expected, budget).map(|_| ()),
        }
    }
}

fn fixture(operation: Operation) -> TestResult<(Cas, Action)> {
    let (mut f, function) = inventory::fixture(false)?;
    let action = match operation {
        Operation::Produce => Action::Produce(function, f.context.clone()),
        Operation::Check => Action::Check(f.context.clone(), f.draft(&function)?),
        Operation::Export | Operation::Replay => {
            let witness = f
                .cas
                .defined_total(&function, &f.context, &mut default_budget()?)?;
            if matches!(operation, Operation::Export) {
                Action::Export(witness)
            } else {
                let bytes = f.cas.export_witness(&witness, &mut default_budget()?)?;
                let (mut receiver, original) = inventory::fixture(true)?;
                let expected = receiver.expected(&original)?;
                return Ok((receiver.cas, Action::Replay(bytes, expected)));
            }
        }
    };
    Ok((f.cas, action))
}

fn attempt(operation: Operation, kind: BudgetKind, amount: u64) -> TestResult<(bool, u64)> {
    let (mut cas, action) = fixture(operation)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = cas.counts();
    let limits = ceiling(kind, amount);
    let mut budget = Budget::new(limits.clone())?;
    budget.work(37, Location::new("prior structural totality Work"))?;
    let result = budget.with_retained_payload(
        19,
        Location::new("prior structural totality payload"),
        |budget| action.run(&mut cas, budget),
    );
    assert_eq!(budget.limits(), &limits);
    if let Err(fault) = &result {
        assert!(
            matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind),
            "{operation:?} {kind:?}: {fault:?}"
        );
        assert_eq!(cas.counts(), counts);
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok((result.is_ok(), budget.usage().work))
}

/// Expectations are sums of independently specified visits, byte lengths and copy allowances.
#[test]
fn defined_total_independent_charge_inventory() -> TestResult {
    let checker = inventory::CHECKER;
    let seal = inventory::seal_work();
    let produce = 256
        + inventory::FRAME
        + inventory::PENDING
        + inventory::PREDICATE
        + checker
        + inventory::PUBLICATION
        + seal;
    let check = 1 + inventory::MEASURE + 256 + checker + inventory::PUBLICATION + seal;
    let bytes = inventory::ENVELOPE + inventory::PAYLOADS.iter().sum::<u64>() + 10 * 11;
    for (operation, cost) in [
        (Operation::Produce, produce),
        (Operation::Check, check),
        (Operation::Export, bytes),
        (Operation::Replay, inventory::replay_work()),
    ] {
        assert_eq!(
            attempt(operation, BudgetKind::Work, cost + 37)?,
            (true, cost + 37),
            "{operation:?} derived Work"
        );
        let final_charge = match operation {
            Operation::Produce | Operation::Check => 4,
            Operation::Export => bytes,
            Operation::Replay => 34,
        };
        assert_eq!(
            attempt(operation, BudgetKind::Work, cost + 36)?,
            (false, cost + 37 - final_charge),
            "{operation:?} derived N-1 prefix"
        );
    }
    let gathered = 16 * inventory::PAYLOADS.iter().sum::<u64>()
        + 256 * inventory::REFERENCES.iter().sum::<u64>();
    for (operation, cost) in [
        (
            Operation::Produce,
            gathered + 5 * 64 + inventory::PENDING + 19,
        ),
        (Operation::Check, gathered + 6 * 64 + 19),
        (Operation::Replay, gathered + 6 * 64 + 19),
        (Operation::Export, bytes + 19),
    ] {
        assert!(
            attempt(operation, BudgetKind::StagedPayloadBytes, cost)?.0,
            "{operation:?} derived staged bytes"
        );
        assert!(
            !attempt(operation, BudgetKind::StagedPayloadBytes, cost - 1)?.0,
            "{operation:?} derived staged N-1"
        );
    }
    Ok(())
}

/// One original leaf and one hypothesis canonicalization consume two condition steps.
/// Witness output is charged only when the sealed bytes are copied to the caller.
#[test]
fn defined_total_other_existing_ceilings() -> TestResult {
    let bytes = inventory::ENVELOPE + inventory::PAYLOADS.iter().sum::<u64>() + 10 * 11;
    for operation in [Operation::Produce, Operation::Check, Operation::Replay] {
        for (kind, needed) in [(BudgetKind::Depth, 1), (BudgetKind::ConditionSteps, 2)] {
            assert!(attempt(operation, kind, needed)?.0);
            assert!(!attempt(operation, kind, needed - 1)?.0);
        }
        assert!(attempt(operation, BudgetKind::OutputBytes, 0)?.0);
    }
    assert!(attempt(Operation::Export, BudgetKind::OutputBytes, bytes)?.0);
    assert!(!attempt(Operation::Export, BudgetKind::OutputBytes, bytes - 1)?.0);
    assert!(attempt(Operation::Export, BudgetKind::Depth, 0)?.0);
    assert!(attempt(Operation::Export, BudgetKind::ConditionSteps, 0)?.0);
    let replay = inventory::replay_work();
    assert_eq!(
        attempt(Operation::Replay, BudgetKind::ReplayWork, replay)?,
        (true, replay + 37)
    );
    assert_eq!(
        attempt(Operation::Replay, BudgetKind::ReplayWork, replay - 1)?,
        (false, replay + 37 - 34)
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum DepthCase {
    Scope,
    Syntax,
}

/// Admission has its ordinary ceilings; the later caller chooses a smaller depth.
fn depth_fixture(case: DepthCase, reordered: bool) -> TestResult<(Fixture, Handle)> {
    let mut f = Fixture::new(Branch::StrictReal, false, 0, reordered)?;
    let function = match case {
        DepthCase::Scope => {
            f.scope = vec![
                vec![AtomicSort::Scalar(Field::Real)],
                vec![AtomicSort::Scalar(Field::Real)],
            ];
            f.region = admit(
                &mut f.cas,
                Draft::Region(RegionDraft {
                    header: Reference::Existing(f.header.clone()),
                    scope: f.scope.clone(),
                    ambient: AmbientDomain::Assignments,
                    expression: RegionExpr::Ambient,
                }),
            )?;
            f.context = admit(
                &mut f.cas,
                Draft::Context(ContextDraft::Root {
                    header: Reference::Existing(f.header.clone()),
                    admissible: Reference::Existing(f.region.clone()),
                    assumptions: Vec::new(),
                }),
            )?;
            f.term(Operator::Bound { depth: 1, slot: 0 }, &[])?
        }
        DepthCase::Syntax => {
            let x = f.term(Operator::Free("x".into()), &[])?;
            let negated = f.term(Operator::Neg, &[x])?;
            f.term(Operator::Neg, &[negated])?
        }
    };
    Ok((f, function))
}

fn depth_action(operation: Operation, case: DepthCase) -> TestResult<(Cas, Action)> {
    let (mut f, function) = depth_fixture(case, false)?;
    let action = match operation {
        Operation::Produce => Action::Produce(function, f.context.clone()),
        Operation::Check => Action::Check(f.context.clone(), f.draft(&function)?),
        Operation::Replay => {
            let witness = f
                .cas
                .defined_total(&function, &f.context, &mut default_budget()?)?;
            let bytes = f.cas.export_witness(&witness, &mut default_budget()?)?;
            let (mut receiver, original) = depth_fixture(case, true)?;
            let expected = receiver.expected(&original)?;
            return Ok((receiver.cas, Action::Replay(bytes, expected)));
        }
        Operation::Export => return Err("depth fixture has no export operation".into()),
    };
    Ok((f.cas, action))
}

fn depth_boundary(case: DepthCase, needed: u64) -> TestResult {
    for operation in [Operation::Produce, Operation::Check, Operation::Replay] {
        for accepted in [true, false] {
            let (mut cas, action) = depth_action(operation, case)?;
            let before = cas.canonical_snapshot(&mut default_budget()?)?;
            let counts = cas.counts();
            let limits = ceiling(BudgetKind::Depth, needed - u64::from(!accepted));
            let mut budget = Budget::new(limits.clone())?;
            budget.work(37, Location::new("prior depth Work"))?;
            let result = action.run(&mut cas, &mut budget);
            assert_eq!(result.is_ok(), accepted, "{case:?} {operation:?}");
            assert_eq!(budget.limits(), &limits);
            assert!(budget.usage().work > 37);
            if let Err(fault) = result {
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::Depth,
                        limit: needed - 1,
                        needed,
                    }
                );
                // Scope refusal precedes shape encoding: only the checkpoint and,
                // for supplied checking, reachability, two measures and five checks.
                match (operation, case) {
                    (Operation::Produce, DepthCase::Scope) => {
                        assert_eq!(budget.usage().work, 37 + 256);
                    }
                    (Operation::Check, DepthCase::Scope) => {
                        assert_eq!(
                            budget.usage().work,
                            37 + 1 + 2 * inventory::MEASURE + 256 + 5
                        );
                    }
                    (Operation::Check, DepthCase::Syntax) => {
                        // Neg: one operator byte, three sort bytes, four scope bytes,
                        // one binding byte, encoded and compared before five checks.
                        assert_eq!(
                            budget.usage().work,
                            37 + 1 + 2 * inventory::MEASURE + 256 + 5 + 2 * 9 + 5
                        );
                    }
                    _ => {}
                }
                assert_eq!(cas.counts(), counts);
                assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
            }
        }
    }
    Ok(())
}

/// Two retained binder frames need depth two even though the bound leaf has syntax depth one.
#[test]
fn defined_total_retained_scope_depth_boundary() -> TestResult {
    depth_boundary(DepthCase::Scope, 2)
}

/// Two unary constructors above a leaf need depth three; the premise-free proof needs only one.
#[test]
fn defined_total_original_syntax_depth_boundary() -> TestResult {
    depth_boundary(DepthCase::Syntax, 3)
}
