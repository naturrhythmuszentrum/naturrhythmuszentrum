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
            24924, 0, 6, 2, 14531, 8993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
        ],
        [
            24920, 0, 4452, 4453, 22459, 4684, 24920, 0, 0, 4684, 4684, 4684, 4684, 0, 1985, 7603,
            0, 0, 0, 0,
        ],
    ),
    (
        0,
        Operation::Check,
        true,
        [
            24924, 0, 6, 2, 14659, 113, 598, 0, 0, 15, 0, 0, 0, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
        ],
        [
            24920, 0, 4452, 4453, 22459, 2095, 24920, 0, 0, 21470, 0, 0, 0, 0, 1985, 7603, 0, 0, 0,
            0,
        ],
    ),
    (
        0,
        Operation::Produce,
        false,
        [
            22857, 0, 3, 2, 14659, 8993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
        ],
        [
            22853, 0, 2385, 2386, 20392, 2617, 22853, 0, 0, 2617, 2617, 2617, 2617, 2617, 2383,
            5536, 0, 0, 0, 0,
        ],
    ),
    (
        0,
        Operation::Produce,
        true,
        [
            22857, 0, 3, 2, 14659, 111, 598, 0, 0, 15, 0, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
        ],
        [
            22853, 0, 2385, 2386, 20392, 396, 22853, 0, 0, 19403, 0, 1133, 1133, 2617, 2383, 5536,
            0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Check,
        false,
        [
            24924, 0, 6, 2, 14531, 8993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
        ],
        [
            24920, 0, 4452, 4453, 22459, 4684, 24920, 0, 0, 4684, 4684, 4684, 4684, 0, 1985, 7603,
            0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Check,
        true,
        [
            24924, 0, 6, 2, 14659, 113, 598, 0, 0, 15, 0, 0, 0, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
        ],
        [
            24920, 0, 4452, 4453, 22459, 2095, 24920, 0, 0, 21470, 0, 0, 0, 0, 1985, 7603, 0, 0, 0,
            0,
        ],
    ),
    (
        1,
        Operation::Produce,
        false,
        [
            22857, 0, 3, 2, 14659, 8993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
        ],
        [
            22853, 0, 2385, 2386, 20392, 2617, 22853, 0, 0, 2617, 2617, 2617, 2617, 2617, 2383,
            5536, 0, 0, 0, 0,
        ],
    ),
    (
        1,
        Operation::Produce,
        true,
        [
            22857, 0, 3, 2, 14659, 111, 598, 0, 0, 15, 0, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
        ],
        [
            22853, 0, 2385, 2386, 20392, 396, 22853, 0, 0, 19403, 0, 1133, 1133, 2617, 2383, 5536,
            0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Check,
        false,
        [
            24924, 0, 6, 2, 14531, 8993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
        ],
        [
            24920, 0, 4452, 4453, 22459, 4684, 24920, 0, 0, 4684, 4684, 4684, 4684, 0, 1985, 7603,
            0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Check,
        true,
        [
            24924, 0, 6, 2, 14659, 113, 598, 0, 0, 15, 0, 0, 0, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
            24924, 24924, 24924, 24924, 24924, 24924, 24924, 24924,
        ],
        [
            24920, 0, 4452, 4453, 22459, 2095, 24920, 0, 0, 21470, 0, 0, 0, 0, 1985, 7603, 0, 0, 0,
            0,
        ],
    ),
    (
        2,
        Operation::Produce,
        false,
        [
            22857, 0, 3, 2, 14659, 8993, 598, 0, 0, 19, 41, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
        ],
        [
            22853, 0, 2385, 2386, 20392, 2617, 22853, 0, 0, 2617, 2617, 2617, 2617, 2617, 2383,
            5536, 0, 0, 0, 0,
        ],
    ),
    (
        2,
        Operation::Produce,
        true,
        [
            22857, 0, 3, 2, 14659, 111, 598, 0, 0, 15, 0, 19, 41, 2, 1, 16, 0, 0, 1, 1,
        ],
        [
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
            22857, 22857, 22857, 22857, 22857, 22857, 22857, 22857,
        ],
        [
            22853, 0, 2385, 2386, 20392, 396, 22853, 0, 0, 19403, 0, 1133, 1133, 2617, 2383, 5536,
            0, 0, 0, 0,
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
            (BudgetKind::Work, 22857, 22890),
            (BudgetKind::StagedPayloadBytes, 14659, 20429),
            (BudgetKind::PayloadBytes, 8993, 2654),
            (BudgetKind::Vertices, 19, 2654),
            (BudgetKind::Edges, 41, 2654),
            (BudgetKind::StagedVertices, 19, 2654),
            (BudgetKind::StagedEdges, 41, 2654),
        ] {
            let prior = if resource == BudgetKind::Work { 37 } else { 0 };
            assert_eq!(
                absent_attempt(input, resource, cost + prior)?,
                (true, 22894)
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
#[test]
fn trig_defined_coverage_request_boundary() -> TestResult {
    assert_eq!(coverage_attempt(203_090)?, (true, 203_090));
    assert_eq!(coverage_attempt(203_089)?, (false, 202_498));
    Ok(())
}
