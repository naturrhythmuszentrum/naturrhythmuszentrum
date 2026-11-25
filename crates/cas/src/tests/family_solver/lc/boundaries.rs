// SPDX-License-Identifier: AGPL-3.0-only
//! Changed LC Work figures retain exact and one-below evidence on fresh original fixtures.

use crate::budget::{Budget, BudgetKind};
use crate::error::{Fault, FaultKind};
use crate::request::Outcome;
use crate::testing::common::TestResult;

/// Keep every unrelated limit from the existing fixture profile.
fn capped(work: u64) -> TestResult<Budget> {
    let mut limits = super::ledger()?.limits().clone();
    limits.work = work;
    Ok(crate::budget::Budget::new(limits)?)
}

/// An exact cap completes; one below fails specifically for the missing final Work.
fn checked(result: TestResult, used: u64, required: u64, short: u64) -> TestResult {
    if short == 0 {
        result?;
        assert_eq!(used, required);
    } else {
        let error = result.err().ok_or("one-below LC operation completed")?;
        let fault = error.downcast_ref::<Fault>().ok_or("non-Work LC refusal")?;
        assert!(
            matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, needed, .. } if needed == required),
            "{fault:?}"
        );
    }
    Ok(())
}

/// ´test:crate:stable-collection-lc-request-work-boundaries´
#[test]
fn request_work_boundaries() -> TestResult {
    for (signs, required) in [([1, 1], 823_812), ([0, 1], 458_432), ([1, 0], 456_020)] {
        for short in [0, 1] {
            let (mut source, mut request) = super::fixture::request(signs, true)?;
            request.limits.work = Some(required - short);
            let outcome = source.cas.run(&request);
            let (result, used): (TestResult, u64) = match outcome {
                Outcome::Solved { usage, .. } if signs == [1, 1] => (Ok(()), usage.work),
                Outcome::Unresolved {
                    usage,
                    solve_reason: Some(crate::solving::SolveReason::MissingUnitProof),
                    ..
                } if signs != [1, 1] => (Ok(()), usage.work),
                Outcome::Exhausted { fault, usage, .. } => (Err(fault.into()), usage.work),
                other => return Err(format!("unexpected capped LC outcome: {other:?}").into()),
            };
            checked(result, used, required, short)?;
        }
    }
    Ok(())
}

/// ´test:crate:stable-collection-lc-identity-work-boundaries´
#[test]
fn identity_work_boundaries() -> TestResult {
    for short in [0, 1] {
        let (mut source, request) = super::fixture::request([1, 1], true)?;
        let claim = super::expected::identity(&mut source, &request)?;
        let mut checking = capped(749_474 - short)?;
        let result =
            super::identity::prove(&mut source, &claim, &mut super::ledger()?, &mut checking);
        checked(result.map(|_| ()), checking.usage().work, 749_474, short)?;
    }
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let claim = super::expected::identity(&mut source, &request)?;
    let witness = super::identity::prove(
        &mut source,
        &claim,
        &mut super::ledger()?,
        &mut super::ledger()?,
    )?;
    let bytes = source.cas.export_witness(&witness, &mut super::ledger()?)?;
    for short in [0, 1] {
        let (mut target, request) = super::fixture::request([1, 1], true)?;
        let claim = super::expected::identity(&mut target, &request)?;
        let mut budget = capped(375_236 - short)?;
        let result = target.cas.replay(&bytes, &claim, &mut budget);
        checked(
            result.map(|_| ()).map_err(Into::into),
            budget.usage().work,
            375_236,
            short,
        )?;
    }
    Ok(())
}

/// ´test:crate:stable-collection-lc-cold-cache-work-boundary´
#[test]
fn cold_cache_work_boundary() -> TestResult {
    let (mut source, request) = super::fixture::request([1, 1], true)?;
    let Outcome::Solved { answer, .. } = source.cas.run(&request) else {
        return Err("LC source".into());
    };
    let bytes = source.cas.export_witness(
        answer.coverage.as_ref().ok_or("LC coverage")?,
        &mut super::ledger()?,
    )?;
    for short in [0, 1] {
        let (mut target, request) = super::fixture::request([1, 1], true)?;
        let set =
            crate::testing::replay_answer::proposal(&source.cas, &answer.set, &mut target.cas)?;
        let expected = super::replay::expected(&target, &request, &set)?;
        let mut cache = crate::proof::AnonymousLemmaCache::new();
        let mut budget = capped(1_599_398 - short)?;
        let result = cache.replay(&mut target.cas, &bytes, &expected, &mut budget);
        checked(
            result.map(|_| ()).map_err(Into::into),
            budget.usage().work,
            1_599_398,
            short,
        )?;
    }
    Ok(())
}

/// ´test:crate:stable-collection-lc-checker-work-boundary´
#[test]
fn checker_work_boundary() -> TestResult {
    for short in [0, 1] {
        let (mut source, request) = super::fixture::request([1, 1], true)?;
        let mut checking = capped(1_079_918 - short)?;
        let result = crate::solving::solve_search::produce(
            &mut source.cas,
            &request,
            &mut crate::solving::radicals::route::Audit::default(),
            &mut super::ledger()?,
            &mut checking,
        );
        let result: TestResult = match result {
            Ok(produced) => produced.fault.map_or(Ok(()), |fault| Err(fault.into())),
            Err(crate::solving::coefficients::model::CoefficientFailure::Fault(fault)) => {
                Err(fault.into())
            }
            Err(other) => Err(other.into()),
        };
        checked(result, checking.usage().work, 1_079_918, short)?;
    }
    Ok(())
}

/// A semantic refusal remains reachable at its exact Work cost, with one-below exhaustion.
pub(super) fn refusal(
    required: u64,
    expected: &Fault,
    mut operation: impl FnMut(&mut Budget) -> Result<(), Fault>,
) -> TestResult {
    for short in [0, 1] {
        let mut budget = capped(required - short)?;
        let fault = operation(&mut budget)
            .err()
            .ok_or("invalid LC input accepted")?;
        if short == 0 {
            assert_eq!(&fault, expected);
            assert_eq!(budget.usage().work, required);
        } else {
            assert!(
                matches!(fault.kind, FaultKind::Exhausted { budget: BudgetKind::Work, needed, .. } if needed == required),
                "{fault:?}"
            );
        }
    }
    Ok(())
}
