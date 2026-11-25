// SPDX-License-Identifier: AGPL-3.0-only
//! Literal complete-operation totality boundaries retain prior Work and live payload.

use crate::testing::common::TestResult;
use crate::testing::trig_defined_work::{Operation, Row, verify};

const ROWS: [Row; 12] = [
    (
        0,
        Operation::Check,
        false,
        [
            24_946, 0, 6, 2, 14_531, 8_993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983,
            24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983,
        ],
        [
            24_979, 0, 4_499, 4_500, 22_518, 4_737, 24_979, 0, 0, 4_737, 4_737, 4_737, 4_737, 37,
            2_022, 7_662, 0, 0, 0, 0,
        ],
    ),
    (
        0,
        Operation::Check,
        true,
        [
            24_924, 0, 6, 2, 14_659, 113, 598, 0, 0, 15, 0, 0, 0, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961,
            24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961,
        ],
        [
            24_957, 0, 4_489, 4_490, 22_496, 2_132, 24_957, 0, 0, 21_507, 0, 0, 0, 37, 2_022,
            7_640, 0, 0, 0, 0,
        ],
    ),
    (
        0,
        Operation::Produce,
        false,
        [
            22_869, 0, 3, 2, 14_659, 8_993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906,
            22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906,
        ],
        [
            22_902, 0, 2_422, 2_423, 20_441, 2_660, 22_902, 0, 0, 2_660, 2_660, 2_660, 2_660,
            2_666, 2_420, 5_585, 0, 0, 0, 0,
        ],
    ),
    (
        0,
        Operation::Produce,
        true,
        [
            22_857, 0, 3, 2, 14_659, 111, 598, 0, 0, 15, 0, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894,
            22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894,
        ],
        [
            22_890, 0, 2_422, 2_423, 20_429, 433, 22_890, 0, 0, 19_440, 0, 1_170, 1_170, 2_654,
            2_420, 5_573, 0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Check,
        false,
        [
            24_946, 0, 6, 2, 14_531, 8_993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983,
            24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983,
        ],
        [
            24_979, 0, 4_499, 4_500, 22_518, 4_737, 24_979, 0, 0, 4_737, 4_737, 4_737, 4_737, 37,
            2_022, 7_662, 0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Check,
        true,
        [
            24_924, 0, 6, 2, 14_659, 113, 598, 0, 0, 15, 0, 0, 0, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961,
            24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961,
        ],
        [
            24_957, 0, 4_489, 4_490, 22_496, 2_132, 24_957, 0, 0, 21_507, 0, 0, 0, 37, 2_022,
            7_640, 0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Produce,
        false,
        [
            22_869, 0, 3, 2, 14_659, 8_993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906,
            22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906,
        ],
        [
            22_902, 0, 2_422, 2_423, 20_441, 2_660, 22_902, 0, 0, 2_660, 2_660, 2_660, 2_660,
            2_666, 2_420, 5_585, 0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Produce,
        true,
        [
            22_857, 0, 3, 2, 14_659, 111, 598, 0, 0, 15, 0, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894,
            22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894,
        ],
        [
            22_890, 0, 2_422, 2_423, 20_429, 433, 22_890, 0, 0, 19_440, 0, 1_170, 1_170, 2_654,
            2_420, 5_573, 0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Check,
        false,
        [
            24_946, 0, 6, 2, 14_531, 8_993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983,
            24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983, 24_983,
        ],
        [
            24_979, 0, 4_499, 4_500, 22_518, 4_737, 24_979, 0, 0, 4_737, 4_737, 4_737, 4_737, 37,
            2_022, 7_662, 0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Check,
        true,
        [
            24_924, 0, 6, 2, 14_659, 113, 598, 0, 0, 15, 0, 0, 0, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961,
            24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961, 24_961,
        ],
        [
            24_957, 0, 4_489, 4_490, 22_496, 2_132, 24_957, 0, 0, 21_507, 0, 0, 0, 37, 2_022,
            7_640, 0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Produce,
        false,
        [
            22_869, 0, 3, 2, 14_659, 8_993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906,
            22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906, 22_906,
        ],
        [
            22_902, 0, 2_422, 2_423, 20_441, 2_660, 22_902, 0, 0, 2_660, 2_660, 2_660, 2_660,
            2_666, 2_420, 5_585, 0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Produce,
        true,
        [
            22_857, 0, 3, 2, 14_659, 111, 598, 0, 0, 15, 0, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894,
            22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894, 22_894,
        ],
        [
            22_890, 0, 2_422, 2_423, 20_429, 433, 22_890, 0, 0, 19_440, 0, 1_170, 1_170, 2_654,
            2_420, 5_573, 0, 0, 0, 0,
        ],
    ),
];

/// Export prepays its copy; checking, production and replay retain completed incremental charges.
/// Bit floors reject invalid zero policies before the operation; other zero ceilings execute.
/// ´test:integration:trig-defined-checking-resource-boundaries´
#[test]
fn trig_defined_checking_resource_boundaries() -> TestResult {
    for row in ROWS {
        verify(row)?;
    }
    Ok(())
}

/// Output-absent production charges predicate admission and retains every consumed prefix.
/// The existing interning-hit rows remain independent controls beside these rows.
/// ´test:integration:trig-defined-absent-output-boundaries´
#[test]
fn trig_defined_absent_output_boundaries() -> TestResult {
    use crate::testing::trig_defined_work::absent_attempt;
    use nrz_cas::limits::BudgetKind;
    for input in 0..3 {
        for (resource, cost, prefix) in [
            (BudgetKind::Work, 22875, 22908),
            (BudgetKind::StagedPayloadBytes, 14659, 20447),
            (BudgetKind::PayloadBytes, 8993, 2666),
            (BudgetKind::Vertices, 19, 2666),
            (BudgetKind::Edges, 41, 2666),
            (BudgetKind::StagedVertices, 19, 2666),
            (BudgetKind::StagedEdges, 41, 2666),
        ] {
            let prior = if resource == BudgetKind::Work { 37 } else { 0 };
            assert_eq!(
                absent_attempt(input, resource, cost + prior)?,
                (true, 22912)
            );
            assert_eq!(
                absent_attempt(input, resource, cost + prior - 1)?,
                (false, prefix)
            );
        }
    }
    Ok(())
}

fn coverage_attempt(work: u64) -> TestResult<(bool, u64)> {
    use crate::testing::cases::Fixture;
    use nrz_cas::request::{Outcome, Task};
    use nrz_cas::syntax::Operator;
    use nrz_cas::{FaultKind, limits::BudgetKind};
    let mut f = Fixture::new()?;
    let original = f.term(Operator::Mul, &[f.x.clone(), f.x.clone()])?;
    let first = f.term(Operator::Sine, &[original])?;
    let second = f.term(Operator::Cosine, &[first])?;
    let last = f.term(Operator::Arctangent, &[second])?;
    let mut request = f.request(
        Task::Coverage {
            domain: f.domain.clone(),
        },
        &[last],
    )?;
    request.limits.work = Some(work);
    match f.cas.run(&request) {
        Outcome::Proved { result, usage, .. } => {
            assert!(result.witness.is_discharged());
            let citations = f.cas.proof_citations(
                result.witness.proof(),
                &mut crate::testing::common::default_budget()?,
            )?;
            assert_eq!(
                citations
                    .iter()
                    .filter(|node| node.primitive == nrz_cas::proof::Primitive::TrigDefined)
                    .count(),
                3
            );
            Ok((true, usage.work))
        }
        Outcome::Exhausted {
            fault, usage, best, ..
        } => {
            assert!(best.is_none());
            assert!(matches!(
                fault.kind,
                FaultKind::Exhausted {
                    budget: BudgetKind::Work,
                    ..
                }
            ));
            Ok((false, usage.work))
        }
        other => Err(format!("coverage outcome: {other:?}").into()),
    }
}

/// The complete request owns a fresh ledger and incrementally charges the three-node chain.
/// ´test:integration:trig-defined-coverage-request-boundary´
/// The request ledger gains 53 Work: 12 retained-kind counts, 2 cost-policy
/// map insertion Work, 18 identity insertions, 3 retained-kind insertions,
/// and 18 vertex admissions. Lookup floors displace the former graph lookup
/// floors equally. Other `Cas::run` ledgers are excluded from this receipt.
#[test]
fn trig_defined_coverage_request_boundary() -> TestResult {
    assert_eq!(coverage_attempt(203_203)?, (true, 203_203));
    assert_eq!(coverage_attempt(203_202)?, (false, 202_611));
    Ok(())
}
