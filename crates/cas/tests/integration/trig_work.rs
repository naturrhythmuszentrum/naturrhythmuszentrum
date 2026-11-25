// SPDX-License-Identifier: AGPL-3.0-only
//! Complete graph operations retain caller usage and roll back every exhausted publication.

use crate::testing::common::{TestResult, default_budget};
use crate::testing::trig::{Fixture, operators};
use crate::testing::trig_limits::{KINDS, boundary, limits};
use crate::testing::trig_replay::{prepared as replay_fixture, witness};
use nrz_cas::limits::{Budget, BudgetKind};
use nrz_cas::proof::{Judgment, VerifiedWitness};
use nrz_cas::syntax::{Draft, Operator, Substitution};
use nrz_cas::{Cas, Fault, FaultKind, Handle, Location};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Node,
    Source,
    Substitute,
    Construct,
    Walk,
    Export,
    Replay,
}

/// Work, depth, staged payload, payload, witness, output, source, occurrences, replay Work.
/// Work excludes 37 prior units; staged payload includes 19 retained caller bytes.
/// A zero records successful execution with that allowance absent.
const COSTS: [(Operation, bool, [u64; 9]); 10] = [
    (
        Operation::Node,
        false,
        [998, 3, 3_942, 3_373, 0, 0, 0, 0, 0],
    ),
    (
        Operation::Source,
        false,
        [648, 0, 3_215, 2_688, 0, 0, 8, 0, 0],
    ),
    (Operation::Source, true, [606, 0, 3_279, 20, 0, 0, 8, 0, 0]),
    (
        Operation::Substitute,
        false,
        [9_978, 2, 8_545, 3_831, 0, 0, 0, 0, 0],
    ),
    (
        Operation::Substitute,
        true,
        [9_871, 2, 8_545, 50, 0, 0, 0, 0, 0],
    ),
    (
        Operation::Construct,
        false,
        [8_207, 4, 10_336, 7_477, 0, 0, 0, 7, 0],
    ),
    (
        Operation::Walk,
        false,
        [17_856, 4, 2_867, 164, 0, 880, 0, 7, 0],
    ),
    (
        Operation::Export,
        false,
        [661, 0, 680, 0, 661, 661, 0, 0, 0],
    ),
    (
        Operation::Replay,
        false,
        [53_245, 4, 14_931, 7_666, 661, 0, 0, 0, 53_245],
    ),
    (
        Operation::Replay,
        true,
        [53_235, 4, 15_059, 228, 661, 0, 0, 0, 53_235],
    ),
];

enum Action {
    Admit(Draft),
    Substitute(Handle, Substitution),
    Construct(Handle, Handle),
    Walk(Handle),
    Export(VerifiedWitness),
    Replay(Vec<u8>, Judgment),
}

impl Action {
    fn run(&self, cas: &mut Cas, budget: &mut Budget) -> Result<(), Fault> {
        match self {
            Self::Admit(draft) => cas.admit(std::slice::from_ref(draft), budget).map(|_| ()),
            Self::Substitute(root, replacement) => cas
                .substitute(root, std::slice::from_ref(replacement), budget)
                .map(|_| ()),
            Self::Construct(root, source) => {
                cas.materialize_original(root, source, budget).map(|_| ())
            }
            Self::Walk(root) => cas
                .walk_original(root, budget)
                .map(|_| ())
                .map_err(|failure| failure.fault),
            Self::Export(proof) => cas.export_witness(proof, budget).map(|_| ()),
            Self::Replay(bytes, expected) => cas.replay(bytes, expected, budget).map(|_| ()),
        }
    }
}

fn fixture(operation: Operation, operator: Operator, warm: bool) -> TestResult<(Cas, Action)> {
    let (mut cas, action) = match operation {
        Operation::Export | Operation::Replay => {
            let (origin, proof) = witness(operator.clone())?;
            if matches!(operation, Operation::Export) {
                (origin, Action::Export(proof))
            } else {
                let bytes = origin.export_witness(&proof, &mut default_budget()?)?;
                let (destination, expected) = replay_fixture(operator, true)?;
                (destination, Action::Replay(bytes, expected))
            }
        }
        _ => graph_fixture(operation, operator)?,
    };
    if warm {
        action.run(&mut cas, &mut default_budget()?)?;
    }
    Ok((cas, action))
}

fn graph_fixture(operation: Operation, operator: Operator) -> TestResult<(Cas, Action)> {
    let mut f = Fixture::new()?;
    let source = Draft::Source {
        key: "trig".into(),
        text: "original".into(),
    };
    if matches!(operation, Operation::Source) {
        return Ok((f.cas, Action::Admit(source)));
    }
    if matches!(operation, Operation::Substitute) {
        let root = f.term(operator, &[f.x.clone()])?;
        let y = f.term(Operator::Free("y".into()), &[])?;
        return Ok((
            f.cas,
            Action::Substitute(
                root,
                Substitution {
                    key: "x".into(),
                    replacement: y,
                },
            ),
        ));
    }
    let projection = f.term(Operator::RealPart, &[f.z.clone()])?;
    if matches!(operation, Operation::Node) {
        let draft = f.draft(operator, &[projection]);
        return Ok((f.cas, Action::Admit(Draft::Expression(draft))));
    }
    let node = f.term(operator, &[projection])?;
    let root = f.term(Operator::Add, &[node.clone(), node])?;
    let source = f
        .cas
        .admit(&[source], &mut default_budget()?)?
        .pop()
        .ok_or("source")?;
    let action = if matches!(operation, Operation::Walk) {
        Action::Walk(
            f.cas
                .materialize_original(&root, &source, &mut default_budget()?)?,
        )
    } else {
        Action::Construct(root, source)
    };
    Ok((f.cas, action))
}

fn attempt(
    operation: Operation,
    operator: Operator,
    warm: bool,
    kind: BudgetKind,
    amount: u64,
) -> TestResult<(bool, u64)> {
    let (mut cas, action) = fixture(operation, operator, warm)?;
    let before = cas.canonical_snapshot(&mut default_budget()?)?;
    let counts = cas.counts();
    let ceilings = limits(kind, amount)?;
    let mut budget = nrz_cas::limits::Budget::new(ceilings.clone())?;
    budget.work(37, Location::new("prior trig lifecycle Work"))?;
    let result =
        budget.with_retained_payload(19, Location::new("prior trig retained payload"), |budget| {
            action.run(&mut cas, budget)
        });
    assert_eq!(budget.limits(), &ceilings);
    assert!(budget.usage().work >= 37);
    if let Err(fault) = &result {
        assert!(
            matches!(fault.kind, FaultKind::Exhausted { budget, .. } if budget == kind),
            "{fault:?}"
        );
        assert!(budget.usage().work <= ceilings.work);
        assert_eq!(cas.counts(), counts);
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    if matches!(operation, Operation::Walk | Operation::Export) {
        assert_eq!(cas.canonical_snapshot(&mut default_budget()?)?, before);
    }
    Ok((result.is_ok(), budget.usage().work))
}

/// Literal complete-operation boundaries preserve prior usage and atomic refusal.
/// ´test:integration:trig-lifecycle-boundaries´
#[test]
fn trig_lifecycle_boundaries() -> TestResult {
    for (operator, _) in operators() {
        for (operation, warm, costs) in COSTS {
            for (kind, cost) in KINDS.into_iter().zip(costs) {
                boundary(kind, cost, |amount| {
                    attempt(operation, operator.clone(), warm, kind, amount)
                })?;
            }
        }
    }
    Ok(())
}
