// SPDX-License-Identifier: AGPL-3.0-only
//! Independent operation fixtures retain prior caller Work, payload and complete rollback evidence.

use super::common::{TestResult, default_budget};
use super::half_pi::Fixture;
use super::half_pi_limits::{KINDS, ceiling};
use super::trig_values::{atan, circular};
use crate::test_api::limits::{Budget, BudgetKind};
use crate::test_api::proof::{ApplicationDraft, Judgment, Statement, VerifiedWitness};
use crate::test_api::{Cas, Fault, FaultKind, Handle, InvalidCode, Location};

/// Primitive index, operation, warmth, ceilings, and whole-call Work receipts and prefixes.
pub(crate) type Row = (usize, Operation, bool, [u64; 20], [u64; 20], [u64; 20]);

/// Zero ordinary ceilings execute; bit policies have a mandatory one-bit floor.
pub(crate) fn verify((index, operation, warm, costs, receipts, prefixes): Row) -> TestResult {
    let kinds = KINDS
        .into_iter()
        .chain([BudgetKind::ComponentBits, BudgetKind::IntermediateBits]);
    for (((kind, cost), receipt), prefix) in kinds.zip(costs).zip(receipts).zip(prefixes) {
        let bits = matches!(
            kind,
            BudgetKind::ComponentBits | BudgetKind::IntermediateBits
        );
        let amount =
            if bits { cost.max(1) } else { cost } + if kind == BudgetKind::Work { 37 } else { 0 };
        assert_eq!(
            attempt(index, operation, warm, kind, amount)?,
            (true, receipt),
            "{index} {operation:?} {warm} {kind:?} at N"
        );
        if bits && cost <= 1 {
            // No operation runs or prepays at zero: the policy itself is invalid.
            let fault = Budget::new(ceiling(kind, 0))
                .err()
                .ok_or("zero bit policy accepted")?;
            assert!(matches!(
                fault.kind,
                FaultKind::Invalid(InvalidCode::Policy)
            ));
            assert_eq!(prefix, 0);
        } else if cost > 0 {
            assert_eq!(
                attempt(index, operation, warm, kind, amount - 1)?,
                (false, prefix),
                "{index} {operation:?} {warm} {kind:?} at N-1"
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operation {
    Check,
    Produce,
    Export,
    Replay,
}

enum Action {
    Check(Handle, Vec<ApplicationDraft>),
    Produce(Handle, Vec<Handle>, Judgment),
    Export(VerifiedWitness),
    Replay(Vec<u8>, Judgment),
}

impl Action {
    fn run(&self, cas: &mut Cas, budget: &mut Budget) -> Result<(), Fault> {
        match self {
            Self::Check(context, steps) => cas.check_proof(context, steps, budget).map(|_| ()),
            Self::Produce(function, premises, expected) => {
                let value = cas.trig_special_value(function, premises, budget)?;
                assert_eq!(
                    value.as_ref().map(VerifiedWitness::judgment),
                    Some(expected)
                );
                Ok(())
            }
            Self::Export(witness) => cas.export_witness(witness, budget).map(|_| ()),
            Self::Replay(bytes, expected) => cas.replay(bytes, expected, budget).map(|_| ()),
        }
    }
}

fn prepared(kind: usize, reordered: bool) -> TestResult<Fixture> {
    match kind {
        0 => circular(
            "-18446744073709551617",
            "-18446744073709551617/2",
            false,
            -1,
            false,
            reordered,
        ),
        1 => circular(
            "-18446744073709551617",
            "-18446744073709551617/2",
            true,
            0,
            false,
            reordered,
        ),
        2 => atan("0", false, reordered),
        _ => atan("1", true, reordered),
    }
}

fn fixture(kind: usize, operation: Operation, warm: bool) -> TestResult<(Cas, Action)> {
    let mut f = prepared(kind, false)?;
    let (mut cas, action) = match operation {
        Operation::Check => (f.cas, Action::Check(f.context, f.steps)),
        Operation::Produce => {
            let mut premises = Vec::new();
            if kind < 2 {
                let steps = f.steps.get(..3).ok_or("half-Pi premises")?;
                premises.push(
                    f.cas
                        .check_proof(&f.context, steps, &mut default_budget()?)?
                        .proof()
                        .clone(),
                );
            } else {
                for step in f.steps.get(..2).ok_or("atan premises")? {
                    premises.push(
                        f.cas
                            .check_proof(
                                &f.context,
                                std::slice::from_ref(step),
                                &mut default_budget()?,
                            )?
                            .proof()
                            .clone(),
                    );
                }
            }
            let Statement::Equality { left, .. } = &f.expected.statement else {
                return Err("equality expectation".into());
            };
            let action = Action::Produce(left.clone(), premises, f.expected);
            (f.cas, action)
        }
        Operation::Export | Operation::Replay => {
            let proof = f
                .cas
                .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
            if operation == Operation::Export {
                (f.cas, Action::Export(proof))
            } else {
                let bytes = f.cas.export_witness(&proof, &mut default_budget()?)?;
                let receiver = prepared(kind, true)?;
                (receiver.cas, Action::Replay(bytes, receiver.expected))
            }
        }
    };
    if warm {
        action.run(&mut cas, &mut default_budget()?)?;
    }
    Ok((cas, action))
}

pub(crate) fn attempt(
    kind: usize,
    operation: Operation,
    warm: bool,
    resource: BudgetKind,
    amount: u64,
) -> TestResult<(bool, u64)> {
    let (mut cas, action) = fixture(kind, operation, warm)?;
    let counts = cas.counts();
    let snapshot = cas.canonical_snapshot(&mut default_budget()?)?;
    let limits = ceiling(resource, amount);
    let mut budget = Budget::new(limits.clone())?;
    budget.work(37, Location::new("prior exact special-value Work"))?;
    let result = budget.with_retained_payload(
        19,
        Location::new("prior exact special-value payload"),
        |budget| action.run(&mut cas, budget),
    );
    assert_eq!(budget.limits(), &limits);
    if let Err(fault) = &result {
        assert!(
            matches!(fault.kind,FaultKind::Exhausted {budget,..} if budget==resource || (resource==BudgetKind::IntermediateBits && budget==BudgetKind::ComponentBits)),
            "{kind} {operation:?} {warm} {resource:?}: {fault:?}"
        );
        assert_eq!(cas.counts(), counts);
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
    }
    if operation == Operation::Export {
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, snapshot);
    }
    Ok((result.is_ok(), budget.usage().work))
}
