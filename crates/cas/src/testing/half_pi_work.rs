// SPDX-License-Identifier: AGPL-3.0-only
//! Complete checking, export and independent receiving-store replay fixtures.

use super::common::{TestResult, default_budget};
use super::half_pi::{original, prepared, term};
use super::half_pi_limits::ceiling;
use crate::test_api::limits::{Budget, BudgetKind};
use crate::test_api::proof::{ApplicationDraft, Judgment, VerifiedWitness};
use crate::test_api::syntax::Operator;
use crate::test_api::{Cas, Fault, FaultKind, Handle, Location, Rational};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operation {
    Check,
    Guarded,
    Export,
    Replay,
}

enum Action {
    Check(Handle, Vec<ApplicationDraft>),
    Export(VerifiedWitness),
    Replay(Vec<u8>, Judgment),
}

impl Action {
    fn run(&self, cas: &mut Cas, budget: &mut Budget) -> Result<(), Fault> {
        match self {
            Self::Check(context, steps) => cas.check_proof(context, steps, budget).map(|_| ()),
            Self::Export(witness) => cas.export_witness(witness, budget).map(|_| ()),
            Self::Replay(bytes, expected) => cas.replay(bytes, expected, budget).map(|_| ()),
        }
    }
}

fn fixture(operation: Operation, warm: bool) -> TestResult<(Cas, Action)> {
    let mut f = prepared(
        "18446744073709551617",
        "18446744073709551617/2",
        false,
        false,
    )?;
    if matches!(operation, Operation::Guarded) {
        f = prepared("0", "0", true, false)?;
        let one = term(
            &mut f.cas,
            &f.header,
            Operator::Rational(Rational::one()),
            &[],
        )?;
        let zero = term(
            &mut f.cas,
            &f.header,
            Operator::Rational(Rational::zero()),
            &[],
        )?;
        let divisor = term(&mut f.cas, &f.header, Operator::Free("x".into()), &[])?;
        let quotient = term(&mut f.cas, &f.header, Operator::Div, &[one, divisor])?;
        let u = term(&mut f.cas, &f.header, Operator::Mul, &[quotient, zero])?;
        original(&mut f, u)?;
    }
    let (mut cas, action) = match operation {
        Operation::Check | Operation::Guarded => (f.cas, Action::Check(f.context, f.steps)),
        Operation::Export | Operation::Replay => {
            let proof = f
                .cas
                .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
            if matches!(operation, Operation::Export) {
                (f.cas, Action::Export(proof))
            } else {
                let bytes = f.cas.export_witness(&proof, &mut default_budget()?)?;
                // The receiving fixture constructs expectations without a destination proof check.
                let destination = prepared(
                    "18446744073709551617",
                    "18446744073709551617/2",
                    false,
                    true,
                )?;
                (destination.cas, Action::Replay(bytes, destination.expected))
            }
        }
    };
    if warm {
        action.run(&mut cas, &mut default_budget()?)?;
    }
    Ok((cas, action))
}

pub(crate) fn attempt(
    operation: Operation,
    warm: bool,
    kind: BudgetKind,
    amount: u64,
) -> TestResult<(bool, u64)> {
    let (mut cas, action) = fixture(operation, warm)?;
    let counts = cas.counts();
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let limits = ceiling(kind, amount);
    let mut budget = Budget::new(limits.clone())?;
    budget.work(37, Location::new("prior half-Pi Work"))?;
    let result =
        budget.with_retained_payload(19, Location::new("prior half-Pi payload"), |budget| {
            action.run(&mut cas, budget)
        });
    assert_eq!(budget.limits(), &limits);
    if let Err(fault) = &result {
        assert!(
            matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind),
            "{operation:?}, {kind:?}: {fault:?}"
        );
        assert_eq!(cas.counts(), counts);
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    if matches!(operation, Operation::Export) {
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok((result.is_ok(), budget.usage().work))
}

/// Successful receipts at narrowed ceilings differ where existing charges use policy bounds.
/// The literal checker charges the component ceiling; declaration comparison and admission charge the name ceiling.
/// Each cold and warm entry was measured separately, including the coupled intermediate policy.
/// The cold Guarded narrowed successes add 22+2+26+26 = 76 local-entry
/// Work; cold `Replay` `NameBytes` adds 8+2+12+12 = 34. Warm receipts stay at unchanged operation Work.
const RECEIPTS: [(Operation, bool, BudgetKind, u64); 8] = [
    (Operation::Guarded, false, BudgetKind::NameBytes, 104_116),
    (Operation::Guarded, true, BudgetKind::NameBytes, 104_040),
    (Operation::Guarded, false, BudgetKind::ComponentBits, 90_100),
    (Operation::Guarded, true, BudgetKind::ComponentBits, 90_024),
    (
        Operation::Guarded,
        false,
        BudgetKind::IntermediateBits,
        90_100,
    ),
    (
        Operation::Guarded,
        true,
        BudgetKind::IntermediateBits,
        90_024,
    ),
    (Operation::Replay, false, BudgetKind::NameBytes, 66_677),
    (Operation::Replay, true, BudgetKind::NameBytes, 66_643),
];

pub(crate) fn success(operation: Operation, warm: bool, kind: BudgetKind, ordinary: u64) -> u64 {
    RECEIPTS
        .iter()
        .find_map(|(action, heated, resource, work)| {
            (*action == operation && *heated == warm && *resource == kind).then_some(*work)
        })
        .unwrap_or(ordinary)
}
