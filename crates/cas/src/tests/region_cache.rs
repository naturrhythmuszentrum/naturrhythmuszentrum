// SPDX-License-Identifier: AGPL-3.0-only
//! Exact decision reuse remains ledger-local, structural and subordinate to certificate checks.

use crate::budget::BudgetKind;
use crate::domain_model::{RegionDraft, RegionExpr};
use crate::error::{FaultKind, Location};
use crate::graph::{Draft, Handle, Reference};
use crate::model::PredicateKind;
use crate::number::Rational;
use crate::proof_model::{Certificate, Statement};
use crate::region_cache::{partition, partition_at};
use crate::testing::common::TestResult;
use crate::testing::native::{Fixture, admit, budget, predicate};

/// Identical syntax imported with different serials is decided once on the shared ledger.
/// ´test:crate:portable-ledger-identity´
#[test]
fn portable_ledger_identity() -> TestResult {
    let mut first = Fixture::new()?;
    let positive = guarded(&mut first, PredicateKind::Positive)?;
    let mut second = Fixture::new()?;
    empty(&mut second)?;
    let imported = guarded(&mut second, PredicateKind::Positive)?;
    assert_ne!(positive.logical_id(), imported.logical_id());
    let mut ledger = budget()?;
    let expected = partition(&first.cas, &[positive.logical_id()], &mut ledger)?;
    assert_eq!(expected.cuts, vec![Rational::zero()]);
    assert_eq!(expected.membership, vec![vec![false, false, true]]);
    let consumed = ledger.usage().condition_steps;
    assert!(consumed > 0);
    assert_eq!(
        partition(&second.cas, &[imported.logical_id()], &mut ledger)?,
        expected
    );
    assert_eq!(ledger.usage().condition_steps, consumed);
    let mut independent = budget()?;
    assert_eq!(
        partition(&second.cas, &[imported.logical_id()], &mut independent)?,
        expected
    );
    assert_eq!(independent.usage().condition_steps, consumed);
    Ok(())
}

/// Root order, exact extra cuts and distinct syntax under equal local IDs cannot alias.
/// ´test:crate:complete-decision-key´
#[test]
fn complete_decision_key() -> TestResult {
    let mut first = Fixture::new()?;
    let positive = guarded(&mut first, PredicateKind::Positive)?;
    let nonnegative = guarded(&mut first, PredicateKind::NonNegative)?;
    let mut ledger = budget()?;
    let roots = [positive.logical_id(), nonnegative.logical_id()];
    let direct = partition(&first.cas, &roots, &mut ledger)?;
    assert_eq!(
        direct.membership,
        vec![vec![false, false, true], vec![false, true, true]]
    );
    let reversed = partition(
        &first.cas,
        &[nonnegative.logical_id(), positive.logical_id()],
        &mut ledger,
    )?;
    assert_eq!(
        reversed.membership,
        vec![vec![false, true, true], vec![false, false, true]]
    );
    let cut = partition_at(&first.cas, &roots, Some(&Rational::one()), &mut ledger)?;
    assert_eq!(cut.cuts, vec![Rational::zero(), Rational::one()]);
    assert_eq!(
        cut.membership,
        vec![
            vec![false, false, true, true, true],
            vec![false, true, true, true, true]
        ]
    );
    let mut second = Fixture::new()?;
    let other = guarded(&mut second, PredicateKind::NonNegative)?;
    assert_eq!(positive.logical_id(), other.logical_id());
    let distinct = partition(&second.cas, &[other.logical_id()], &mut ledger)?;
    assert_eq!(distinct.membership, vec![vec![false, true, true]]);
    Ok(())
}

/// A warm partition is neither a trusted supplied certificate nor a proof of another relation.
/// ´test:crate:rechecks-untrusted-certificate´
#[test]
fn rechecks_untrusted_certificate() -> TestResult {
    let mut fixture = Fixture::new()?;
    let positive = guarded(&mut fixture, PredicateKind::Positive)?;
    let nonnegative = guarded(&mut fixture, PredicateKind::NonNegative)?;
    let statement = Statement::Subset {
        subset: positive.clone(),
        superset: nonnegative.clone(),
    };
    let mut ledger = budget()?;
    let evidence = crate::region_check::evidence(&mut fixture.cas, &statement, &mut ledger)?;
    let consumed = ledger.usage().condition_steps;
    crate::region_check::decide(
        &mut fixture.cas,
        &statement,
        &Certificate::Region(evidence.clone()),
        &mut ledger,
    )?;
    assert_eq!(ledger.usage().condition_steps, consumed);
    let mut changed = evidence;
    *changed
        .membership
        .first_mut()
        .and_then(|row| row.get_mut(1))
        .ok_or("missing cut")? = true;
    assert!(
        crate::region_check::decide(
            &mut fixture.cas,
            &statement,
            &Certificate::Region(changed),
            &mut ledger
        )
        .is_err()
    );
    let false_statement = Statement::Subset {
        subset: nonnegative,
        superset: positive,
    };
    let exact = crate::region_check::evidence(&mut fixture.cas, &false_statement, &mut ledger)?;
    assert!(
        crate::region_check::decide(
            &mut fixture.cas,
            &false_statement,
            &Certificate::Region(exact),
            &mut ledger
        )
        .is_err()
    );
    Ok(())
}

/// Retained cache bytes count against staging, and a stricter reconstruction policy cannot reuse a hit.
/// ´test:crate:storage-and-policy-boundaries´
#[test]
fn storage_and_policy_boundaries() -> TestResult {
    let fixture = Fixture::new()?;
    let mut ledger = budget()?;
    partition(&fixture.cas, &[fixture.domain.logical_id()], &mut ledger)?;
    let retained = ledger.region_partitions.bytes;
    assert!(retained > 0);
    let available = ledger
        .limits()
        .staged_payload_bytes
        .checked_sub(retained)
        .ok_or("cache bound")?;
    let at = Location::new("region memo test");
    ledger.check(BudgetKind::StagedPayloadBytes, available, at)?;
    assert!(
        ledger
            .check(BudgetKind::StagedPayloadBytes, available + 1, at)
            .is_err()
    );
    let mut strict = ledger.limits().clone();
    strict.condition_steps = ledger.usage().condition_steps;
    let previous = ledger.restrict_storage(&strict);
    let fault = partition(&fixture.cas, &[fixture.domain.logical_id()], &mut ledger)
        .err()
        .ok_or("stricter policy reused a decision")?;
    assert!(matches!(
        fault.kind,
        FaultKind::Exhausted {
            budget: BudgetKind::ConditionSteps,
            ..
        }
    ));
    assert_eq!(ledger.region_partitions.bytes, retained);
    ledger.restore_limits(previous);
    partition(&fixture.cas, &[fixture.domain.logical_id()], &mut ledger)?;
    assert_eq!(ledger.usage().condition_steps, strict.condition_steps);
    Ok(())
}

/// Admit a selector without invoking the region decision code.
fn guarded(fixture: &mut Fixture, kind: PredicateKind) -> TestResult<Handle> {
    let guard = predicate(
        &mut fixture.cas,
        &fixture.header,
        kind,
        std::slice::from_ref(&fixture.x),
    )?;
    admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: crate::domain_model::AmbientDomain::Real("x".into()),
            expression: RegionExpr::GuardTrue {
                base: Reference::Existing(fixture.domain.clone()),
                predicate: Reference::Existing(guard),
            },
        }),
    )
}

/// Shift only region admission serials in the second store.
fn empty(fixture: &mut Fixture) -> TestResult<Handle> {
    admit(
        &mut fixture.cas,
        Draft::Region(RegionDraft {
            header: Reference::Existing(fixture.header.clone()),
            scope: Vec::new(),
            ambient: crate::domain_model::AmbientDomain::Real("x".into()),
            expression: RegionExpr::Empty,
        }),
    )
}
