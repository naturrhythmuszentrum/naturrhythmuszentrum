// SPDX-License-Identifier: AGPL-3.0-only
//! Exact specialized distribution limits use independent hand-counted actions and nodes.

use crate::testing::common::TestResult;
use crate::testing::distributions::{Fixture, output, replay};
use nrz_cas::FaultKind;
use nrz_cas::limits::BudgetKind;
use nrz_cas::request::{ActionMode, Outcome, Task};
use nrz_cas::syntax::Operator;

/// A delta-prime requires order one even when its action has a simple polynomial value.
/// ´test:integration:distribution-delta-order-exact-boundary´
#[test]
fn distribution_delta_order_exact_boundary() -> TestResult {
    for limit in [1, 0] {
        let mut fixture = Fixture::new()?;
        let delta = fixture.delta("2", 1)?;
        let mut request = fixture.base.request(
            Task::Pair {
                space: fixture.space.clone(),
                action_mode: ActionMode::NearSupport {
                    neighborhood: fixture.base.domain.clone(),
                },
            },
            &[delta, fixture.base.x.clone()],
        )?;
        request.limits.delta_order = Some(limit);
        match fixture.base.cas.run(&request) {
            Outcome::Proved { result, .. } if limit == 1 => {
                assert_eq!(output(&result)?, fixture.base.number("-1")?);
                assert_eq!(result.replay_usage.delta_order, 1);
                replay(&fixture, &result)?;
            }
            Outcome::Exhausted {
                fault, best: None, ..
            } if limit == 0 => {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::DeltaOrder,
                        ..
                    }
                ));
            }
            other => return Err(format!("delta-order boundary: {other:?}").into()),
        }
    }
    Ok(())
}

/// One declared test and its first derivative contain exactly two reached test records.
/// ´test:integration:distribution-test-nodes-exact-boundary´
#[test]
fn distribution_test_nodes_exact_boundary() -> TestResult {
    for limit in [2, 1] {
        let mut fixture = Fixture::new()?;
        let test = fixture.test()?;
        let derivative = fixture.node(Operator::TestDerivative { order: 1 }, &[test])?;
        let delta = fixture.delta("2", 0)?;
        let mut request = fixture.base.request(
            Task::Pair {
                space: fixture.space.clone(),
                action_mode: ActionMode::Test,
            },
            &[delta, derivative],
        )?;
        request.limits.test_nodes = Some(limit);
        match fixture.base.cas.run(&request) {
            Outcome::Proved { result, usage, .. } if limit == 2 => {
                assert_eq!(usage.test_nodes, 2);
                assert_eq!(result.replay_usage.test_nodes, 2);
                replay(&fixture, &result)?;
            }
            Outcome::Exhausted {
                fault, best: None, ..
            } if limit == 1 => {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::TestNodes,
                        ..
                    }
                ));
            }
            other => return Err(format!("test-node boundary: {other:?}").into()),
        }
    }
    Ok(())
}

/// Supported action and delta evaluation each run once live and once in independent replay.
/// ´test:integration:distribution-pairing-steps-exact-boundary´
#[test]
fn distribution_pairing_steps_exact_boundary() -> TestResult {
    for limit in [4, 3] {
        let mut fixture = Fixture::new()?;
        let delta = fixture.delta("2", 0)?;
        let mut request = fixture.base.request(
            Task::Pair {
                space: fixture.space.clone(),
                action_mode: ActionMode::NearSupport {
                    neighborhood: fixture.base.domain.clone(),
                },
            },
            &[delta, fixture.base.x.clone()],
        )?;
        request.limits.pairing_steps = Some(limit);
        match fixture.base.cas.run(&request) {
            Outcome::Proved { result, usage, .. } if limit == 4 => {
                assert_eq!(usage.pairing_steps, 2);
                assert_eq!(result.replay_usage.pairing_steps, 4);
                assert_eq!(output(&result)?, fixture.base.number("2")?);
                replay(&fixture, &result)?;
            }
            Outcome::Exhausted {
                fault, best: None, ..
            } if limit == 3 => {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::PairingSteps,
                        ..
                    }
                ));
            }
            other => return Err(format!("pairing-step boundary: {other:?}").into()),
        }
    }
    Ok(())
}
