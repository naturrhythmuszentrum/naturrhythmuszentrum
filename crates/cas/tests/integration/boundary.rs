// SPDX-License-Identifier: AGPL-3.0-only
//! Portable lifecycle and explicit resource boundaries for complete and partial calculus results.

use crate::testing::boundary::{facts, literal, named, point, quadratic, whole};
use crate::testing::budgets::derivation_budget as budget;
use crate::testing::cases::Fixture;
use crate::testing::common::TestResult;
use nrz_cas::limits::BudgetKind;
use nrz_cas::proof::Statement;
use nrz_cas::request::{BoundaryFact, Outcome, ResultPayload, Task};
use nrz_cas::syntax::{CaseName, Operator, PredicateKind};
use nrz_cas::{Cas, FaultKind};

/// Every component survives a fresh store; Q's first derivative includes the actual zero point.
/// ´test:integration:derivative-map-portable-lifecycle´
#[test]
fn derivative_map_portable_lifecycle() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = quadratic(&mut fixture, false)?;
    let Outcome::Proved { result, .. } = whole(&mut fixture, &function, 1)? else {
        return Err("complete Q map expected".into());
    };
    let Some(ResultPayload::DerivativeMap(map)) = result.payload.as_deref() else {
        return Err("map".into());
    };
    assert!(map.complete().is_some());
    assert_eq!(map.order(), 1);
    assert!(
        facts(&result)?
            .iter()
            .any(|fact| matches!(fact.judgment().statement, Statement::DerivativeAt(_)))
    );
    replay_components(&fixture, &result)?;
    assert!(
        result
            .response
            .as_ref()
            .is_some_and(|bytes| bytes.starts_with(b"NRZRUN03"))
    );
    let result = match whole(&mut fixture, &function, 2)? {
        Outcome::Partial { result, .. } => result,
        other => return Err(format!("mixed Q second derivative map expected: {other:?}").into()),
    };
    assert!(facts(&result)?.iter().any(|fact| matches!(&fact.judgment().statement, Statement::DerivativeDoesNotExist(value) if value.order == 2)));
    second_cells(&mut fixture, &result)?;
    replay_components(&fixture, &result)?;
    Ok(())
}

/// An order refusal produces no mathematical nonexistence assertion.
/// ´test:integration:boundary-order-budget-refusal´
#[test]
fn boundary_order_budget_refusal() -> TestResult {
    let mut fixture = Fixture::new()?;
    let function = named(&mut fixture, CaseName::Abs)?;
    let location = fixture.boundary(&fixture.zero.clone())?;
    let mut request = fixture.request(
        Task::Boundary {
            coordinate: "x".into(),
            point: location,
            requested_facts: vec![BoundaryFact::Jets { order: 2 }],
        },
        &[function],
    )?;
    request.limits.jet_order = Some(1);
    assert!(
        matches!(fixture.cas.run(&request), Outcome::Exhausted { fault, best: None, .. }
        if matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::JetOrder, .. }))
    );
    Ok(())
}

/// A missing side or owned point remains pending and preserves independently replayed facts.
/// ´test:integration:boundary-missing-neighborhood-is-pending´
#[test]
fn boundary_missing_neighborhood_is_pending() -> TestResult {
    let mut fixture = Fixture::new()?;
    let coordinate = fixture.x.clone();
    let positive = fixture.predicate(PredicateKind::Positive, std::slice::from_ref(&coordinate))?;
    let right = fixture.region(&positive, true)?;
    let function = fixture.cases(&[(right, coordinate)])?;
    let Outcome::PendingPremise {
        partial: Some(result),
        ..
    } = point(&mut fixture, &function, 1)?
    else {
        return Err("missing side became nonexistence".into());
    };
    assert!(!facts(&result)?.iter().any(|fact| matches!(
        fact.judgment().statement,
        Statement::DerivativeDoesNotExist(_)
    )));
    assert!(
        facts(&result)?
            .iter()
            .any(|fact| matches!(fact.judgment().statement, Statement::Ownership { .. }))
    );
    Ok(())
}

/// Unknown strict branch definedness cannot be advertised as a complete derivative.
/// ´test:integration:boundary-unsupported-branch-preserves-prefix´
#[test]
fn boundary_unsupported_branch_preserves_prefix() -> TestResult {
    let mut fixture = Fixture::new()?;
    let coordinate = fixture.x.clone();
    let unsupported = fixture.term(
        Operator::GeneralPower(nrz_cas::syntax::Branch::StrictReal),
        &[coordinate.clone(), coordinate.clone()],
    )?;
    let nonnegative = fixture.predicate(
        PredicateKind::NonNegative,
        std::slice::from_ref(&coordinate),
    )?;
    let left = fixture.region(&nonnegative, false)?;
    let right = fixture.region(&nonnegative, true)?;
    let function = fixture.cases(&[(left, coordinate), (right, unsupported)])?;
    let outcome = whole(&mut fixture, &function, 1)?;
    let result = match outcome {
        Outcome::PendingPremise {
            partial: Some(result),
            ..
        }
        | Outcome::UnknownTotality {
            partial: Some(result),
            ..
        } => result,
        other => return Err(format!("expected retained partial facts, got {other:?}").into()),
    };
    assert!(
        facts(&result)?
            .iter()
            .any(|fact| matches!(fact.judgment().statement, Statement::BoundaryPartition(_)))
    );
    Ok(())
}

/// Two three-entry jets charge six generated entries and twelve entries across check plus replay.
/// ´test:integration:boundary-jet-entries-exact-refusal-boundary´
#[test]
fn boundary_jet_entries_exact_refusal_boundary() -> TestResult {
    for limit in [12, 11] {
        let mut fixture = Fixture::new()?;
        let function = named(&mut fixture, CaseName::Abs)?;
        let location = fixture.boundary(&fixture.zero.clone())?;
        let mut request = fixture.request(
            Task::Boundary {
                coordinate: "x".into(),
                point: location,
                requested_facts: vec![BoundaryFact::Jets { order: 2 }],
            },
            &[function],
        )?;
        request.limits.jet_entries = Some(limit);
        match fixture.cas.run(&request) {
            Outcome::Proved { result, usage, .. } if limit == 12 => {
                assert_eq!(usage.jet_entries, 6);
                assert_eq!(result.replay_usage.jet_entries, 12);
                assert_eq!(facts(&result)?.len(), 2);
            }
            Outcome::Exhausted {
                best: Some(result),
                fault,
                ..
            } if limit == 11 => {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::JetEntries,
                        ..
                    }
                ));
                assert_eq!(facts(&result)?.len(), 1);
                assert!(matches!(
                    result.witness.judgment().statement,
                    Statement::Jet {
                        side: nrz_cas::domain::Side::Below,
                        ..
                    }
                ));
            }
            other => return Err(format!("unexpected jet ceiling outcome: {other:?}").into()),
        }
    }
    Ok(())
}

/// Complete boundary witnesses ignore unrelated admission serials and retain explicit order on replay.
/// ´test:integration:boundary-canonical-replay-and-expected-order´
#[test]
fn boundary_canonical_replay_and_expected_order() -> TestResult {
    let mut encodings = Vec::new();
    for extra in [false, true] {
        let mut fixture = Fixture::new()?;
        if extra {
            fixture.number("17")?;
        }
        let function = named(&mut fixture, CaseName::Abs)?;
        let Outcome::Nonexistent { result, .. } = point(&mut fixture, &function, 1)? else {
            return Err("abs obstruction".into());
        };
        encodings.push(
            fixture
                .cas
                .export_witness(&result.witness, &mut budget()?)?,
        );
    }
    let [first, second] = encodings.as_slice() else {
        return Err("encodings".into());
    };
    assert_eq!(first, second);
    let mut receiver = Cas::new(budget()?.limits().clone())?;
    let witness = receiver.inspect(first, &mut budget()?)?;
    let mut expected = witness.judgment().clone();
    let Statement::DerivativeDoesNotExist(value) = &mut expected.statement else {
        return Err("nonexistence".into());
    };
    value.order = 2;
    assert!(receiver.replay(first, &expected, &mut budget()?).is_err());
    let before = receiver.counts();
    let truncated = first
        .get(..first.len().saturating_sub(1))
        .ok_or("truncation")?;
    assert!(receiver.inspect(truncated, &mut budget()?).is_err());
    assert_eq!(receiver.counts(), before);
    Ok(())
}

/// Each retained subproof independently imports, even when the request replayed a covering root.
fn replay_components(fixture: &Fixture, result: &nrz_cas::request::SearchResult) -> TestResult {
    for fact in facts(result)? {
        let bytes = fixture.cas.export_witness(fact, &mut budget()?)?;
        let mut receiver = Cas::new(budget()?.limits().clone())?;
        let replayed = receiver.inspect(&bytes, &mut budget()?)?;
        assert_eq!(receiver.export_witness(&replayed, &mut budget()?)?, bytes);
    }
    Ok(())
}

/// The mixed second derivative has independently specified constant values on its two open cells.
fn second_cells(fixture: &mut Fixture, result: &nrz_cas::request::SearchResult) -> TestResult {
    let Some(ResultPayload::DerivativeMap(map)) = result.payload.as_deref() else {
        return Err("second derivative map".into());
    };
    let Statement::BoundaryPartition(partition) = &map.partition().judgment().statement else {
        return Err("partition".into());
    };
    let mut values = 0;
    for fact in map.facts() {
        if let Statement::CaseDerivative(value) = &fact.judgment().statement
            && value.order == 2
            && partition.cells.contains(&value.domain)
        {
            literal(
                fixture,
                &value.derivative,
                ["2", "4"]
                    .get(values)
                    .ok_or("extra second derivative cell")?,
            )?;
            values += 1;
        }
    }
    assert_eq!(values, 2);
    Ok(())
}

/// Two levels generate eight entries; live checks and the three covering roots check twenty-four.
/// ´test:integration:higher-derivative-replay-closure-budget´
#[test]
fn higher_derivative_replay_closure_budget() -> TestResult {
    for limit in [24, 23] {
        let mut fixture = Fixture::new()?;
        let function = quadratic(&mut fixture, false)?;
        let mut request = fixture.request(
            Task::Differentiate {
                coordinate: "x".into(),
                order: 2,
                layer: nrz_cas::domain::Layer::Pointwise,
                domain: fixture.domain.clone(),
            },
            &[function],
        )?;
        request.limits.jet_entries = Some(limit);
        match fixture.cas.run(&request) {
            Outcome::Partial { result, usage, .. } if limit == 24 => {
                assert_eq!(usage.jet_entries, 8);
                assert_eq!(result.replay_usage.jet_entries, 24);
                second_cells(&mut fixture, &result)?;
                assert!(
                    facts(&result)?
                        .iter()
                        .any(|fact| matches!(&fact.judgment().statement,
                    Statement::DerivativeDoesNotExist(value) if value.order == 2))
                );
            }
            Outcome::Exhausted {
                best: Some(result),
                fault,
                ..
            } if limit == 23 => {
                assert!(matches!(
                    fault.kind,
                    FaultKind::Exhausted {
                        budget: BudgetKind::JetEntries,
                        ..
                    }
                ));
                second_cells(&mut fixture, &result)?;
                assert!(
                    !facts(&result)?
                        .iter()
                        .any(|fact| matches!(&fact.judgment().statement,
                    Statement::DerivativeDoesNotExist(value) if value.order == 2))
                );
            }
            other => {
                return Err(
                    format!("unexpected higher derivative replay outcome: {other:?}").into(),
                );
            }
        }
    }
    Ok(())
}
