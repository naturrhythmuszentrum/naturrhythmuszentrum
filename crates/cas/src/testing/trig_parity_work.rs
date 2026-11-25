// SPDX-License-Identifier: AGPL-3.0-only
//! Independent operation fixtures retain prior caller Work, payload and complete rollback evidence.

use super::common::{TestResult, default_budget};
use super::half_pi::Fixture;
use super::half_pi_limits::{KINDS, ceiling};
use super::trig_parity::fixture as syntax;
use crate::test_api::limits::{Budget, BudgetKind};
use crate::test_api::proof::{ApplicationDraft, Judgment, VerifiedWitness};
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
                let value = cas.trig_parity(
                    function,
                    premises.first().ok_or_else(|| {
                        Fault::invalid(InvalidCode::Rule, Location::new("parity fixture"))
                    })?,
                    budget,
                )?;
                assert_eq!(value.judgment(), expected);
                Ok(())
            }
            Self::Export(witness) => cas.export_witness(witness, budget).map(|_| ()),
            Self::Replay(bytes, expected) => cas.replay(bytes, expected, budget).map(|_| ()),
        }
    }
}

fn prepared(kind: usize, reordered: bool) -> TestResult<(Fixture, Handle)> {
    let (f, function, _) = syntax(kind, true, true, reordered)?;
    Ok((f, function))
}

fn fixture(kind: usize, operation: Operation, warm: bool) -> TestResult<(Cas, Action)> {
    let (mut f, function) = prepared(kind, false)?;
    let (mut cas, action) = match operation {
        Operation::Check => (f.cas, Action::Check(f.context, f.steps)),
        Operation::Produce => {
            let guard = f.cas.check_proof(
                &f.context,
                &f.steps[..f.steps.len() - 1],
                &mut default_budget()?,
            )?;
            let action = Action::Produce(function, vec![guard.proof().clone()], f.expected);
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
                let (receiver, _) = prepared(kind, true)?;
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
    budget.work(37, Location::new("prior trigonometric parity Work"))?;
    let result = budget.with_retained_payload(
        19,
        Location::new("prior trigonometric parity payload"),
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

/// No negated operand or parity endpoint is interned before the measured production.
pub(crate) fn absent_attempt(
    kind: usize,
    resource: BudgetKind,
    amount: u64,
) -> TestResult<(bool, u64)> {
    let (mut f, function, original) = super::trig_parity::source(kind)?;
    let guard = f
        .cas
        .check_proof(&f.context, &f.steps, &mut default_budget()?)?;
    let before = f.cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = f.cas.counts();
    let limits = ceiling(resource, amount);
    let mut budget = Budget::new(limits.clone())?;
    budget.work(37, Location::new("prior absent parity Work"))?;
    let result =
        budget.with_retained_payload(19, Location::new("prior absent parity payload"), |budget| {
            f.cas.trig_parity(&function, guard.proof(), budget)
        });
    assert_eq!(budget.limits(), &limits);
    match result {
        Ok(witness) => {
            let produced = f.cas.counts();
            let (left, right) =
                super::trig_parity::endpoints(&mut f.cas, &f.header, kind, &function, &original)?;
            assert_eq!(f.cas.counts(), produced, "expected wrappers were produced");
            assert_eq!(
                witness.judgment().statement,
                crate::test_api::proof::Statement::Equality {
                    left,
                    right,
                    domain: f.domain,
                    domain_contract: crate::test_api::proof::DomainContract::DefinedAndEqual
                }
            );
            assert_eq!(witness.hypotheses(), guard.hypotheses());
            assert_eq!(witness.judgment().residuals, []);
            Ok((true, budget.usage().work))
        }
        Err(fault) => {
            assert!(
                matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == resource),
                "{resource:?}: {fault:?}"
            );
            assert_eq!(f.cas.counts(), counts);
            assert_eq!(f.cas.canonical_snapshot(&mut default_budget()?)?, before);
            Ok((false, budget.usage().work))
        }
    }
}
