// SPDX-License-Identifier: AGPL-3.0-only
//! N/N-1 complete peaks include the original ledger's registrations and caller reservation.
//! Graph G and judgment J are public CAS inventories: these are ledger-composition
//! controls, not independent CAS inventory controls. Omitting a CAS inventory item
//! would lower both the expected ceiling and reservation. A separate whole-graph
//! census would duplicate private vertex payload, interning-key and proof encodings.

use super::{TestResult, bytes, fixture};
use crate::{
    Heap, HeapLimits, LoanError,
    loan::{fault::Injection, retained_expm1::admission},
};
use nrz_bindings::{
    Budget,
    readings::algebra::{
        Exhaustion, FaultKind, Location,
        enclosure::prepared_expm1::{Counts, Policy},
        limits::BudgetKind,
    },
};
use nrz_scalar::ArithmeticContext;

/// Caller bytes are large enough that warm symbolic preparation precedes the isolated boundary.
const PREFIX: u64 = 1_000_000;
/// Explicit original-account Work prefix survives every refusal.
const AT: Location = Location::new("prepared owner boundary fixture");

/// Live producing and unrelated registrations remain additive at the complete managed peak.
#[test]
fn retained_law_ledger_composition_staged_boundary() -> TestResult {
    let (mut cas, request) = fixture::request()?;
    let context = ArithmeticContext::new()?;
    let parts = crate::prepared_expm1::owned_parts(
        &mut cas,
        &request,
        [-2.0, 0.0],
        Policy::default(),
        &context,
        &mut Budget::new(fixture::limits())?,
        &mut Counts::default(),
        0,
    )?;
    let evidence = bytes::evidence(&parts.authority)?;
    let [header, use_header, row] = bytes::headers();
    let staged = u64::try_from(header + evidence + use_header + 2 * row)?;
    let graph = cas.counts().payload_bytes;
    let judgments = cas.retained_judgment_bytes();
    let (mut other, other_request) = fixture::request()?;
    other.prepare_expm1(
        &other_request,
        Policy::default(),
        &mut Budget::new(fixture::limits())?,
        &mut Counts::default(),
    )?;
    let unrelated = other.retained_judgment_bytes() + 64;
    for registered in [false, true] {
        for foreign in [false, true] {
            let registered_judgments = u64::from(registered) * judgments;
            let persistent =
                u64::from(registered) * (judgments + 64) + u64::from(foreign) * unrelated;
            let boundary = PREFIX + (graph - registered_judgments) + persistent + staged;
            for short in [0, 1] {
                let mut budget = Budget::new(nrz_bindings::Limits {
                    staged_payload_bytes: boundary - short,
                    ..fixture::limits()
                })?;
                if foreign {
                    other.prepare_expm1(
                        &other_request,
                        Policy::default(),
                        &mut budget,
                        &mut Counts::default(),
                    )?;
                }
                if registered {
                    cas.prepare_expm1(
                        &request,
                        Policy::default(),
                        &mut budget,
                        &mut Counts::default(),
                    )?;
                }
                budget.work(7, AT)?;
                let before = budget.usage().work;
                let mut heap = Heap::new(HeapLimits::default())?;
                let injection = Injection::after(usize::MAX);
                let result = budget.with_retained_payload(PREFIX, AT, |budget| {
                    cas.with_retained_graph(budget, |budget| {
                        admission::shells(
                            &mut heap,
                            17,
                            u64::try_from(evidence).map_err(|_| LoanError::ByteOverflow)?,
                            budget,
                            fixture::seal(),
                        )
                    })
                });
                if short == 0 {
                    drop(result?);
                    assert_eq!(Injection::attempts(), 3);
                    assert_eq!(Injection::prefix(), [3, 1, 0]);
                } else {
                    let error = result.err().ok_or("complete managed peak must refuse")?;
                    let LoanError::Budget(fault) = error else {
                        return Err(format!("{error:?}").into());
                    };
                    assert_eq!(
                        fault.kind,
                        FaultKind::Exhausted {
                            reason: Exhaustion::Limit,
                            budget: BudgetKind::StagedPayloadBytes,
                            limit: persistent + staged - 1,
                            needed: persistent + staged
                        }
                    );
                    assert_eq!(Injection::attempts(), 0);
                    assert_eq!(heap.loans.debits.capacity(), 0);
                }
                assert!(heap.loans.debits.iter().all(Option::is_none));
                assert!(budget.usage().work > before);
                assert_eq!(budget.limits().staged_payload_bytes, boundary - short);
                drop(injection);
            }
        }
    }
    Ok(())
}
/// Physical retained ceilings refuse before headers even when the ledger staged check passes.
#[test]
fn retained_law_common_preflight_boundary() -> TestResult {
    let [h, u, row] = bytes::headers();
    let e = 1234;
    let n = h + e + u + 2 * row;
    for short in [0, 1] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut budget = Budget::new(nrz_bindings::Limits {
            output_bytes: u64::try_from(n - short)?,
            ..fixture::limits()
        })?;
        let injection = Injection::after(usize::MAX);
        let result = admission::shells(
            &mut heap,
            17,
            u64::try_from(e)?,
            &mut budget,
            fixture::seal(),
        );
        if short == 0 {
            drop(result?);
            assert_eq!(Injection::attempts(), 3);
        } else {
            assert!(matches!(
                result,
                Err(LoanError::Heap(crate::HeapError::ResourceBytes {
                    staged: false,
                    ..
                }))
            ));
            assert_eq!(Injection::attempts(), 0);
        }
        drop(injection);
    }
    Ok(())
}
/// Failed managed allocation closes only its acquired prefix and installs no partial law.
#[test]
fn retained_law_allocation_prefixes() -> TestResult {
    for prefix in 0..3 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let (cas, request) = fixture::request()?;
        let injection = Injection::after(prefix);
        let error = heap
            .prepare_retained_expm1(
                cas,
                &request,
                [-2.0, 0.0],
                Policy::default(),
                &ArithmeticContext::new()?,
                &mut Counts::default(),
                &mut account,
            )
            .expect_err("injected allocation");
        assert!(matches!(
            error.0,
            crate::RetainedExpm1Error::Ownership(LoanError::AllocationRefused)
        ));
        assert_eq!(Injection::attempts(), prefix + 1);
        let retained = usize::from(prefix != 0);
        assert_eq!(Injection::prefix(), [prefix, retained, prefix - retained]);
        assert!(heap.loans.debits.iter().all(Option::is_none));
        assert_eq!(heap.loans.debits.capacity(), 2 * retained);
        assert!(error.1.retained_judgment_count() > 0);
        drop(injection);
    }
    Ok(())
}

/// Further-use admission pays U and actual replacement only, alongside active ledger owners.
#[test]
fn retained_law_additional_use_combined_boundary() -> TestResult {
    let [_, u, row] = bytes::headers();
    let (mut other, request) = fixture::request()?;
    other.prepare_expm1(
        &request,
        Policy::default(),
        &mut Budget::new(fixture::limits())?,
        &mut Counts::default(),
    )?;
    let m = other.retained_judgment_bytes() + 64;
    for spare in [false, true] {
        let staged = if spare { u } else { u + row + 4 * row };
        let n = PREFIX + 2 * m + u64::try_from(staged)?;
        for short in [0, 1] {
            let mut account = crate::ExecutionAccount::new(Budget::new(nrz_bindings::Limits {
                staged_payload_bytes: n - short,
                ..fixture::limits()
            })?)?;
            let mut heap = Heap::new(HeapLimits::default())?;
            // Cold preparation need not install a persistent registration. Seed the graph
            // separately, then the constructor's real warm preparation registers it here.
            let (mut cas, original) = fixture::request()?;
            cas.prepare_expm1(
                &original,
                Policy::default(),
                &mut Budget::new(fixture::limits())?,
                &mut Counts::default(),
            )?;
            let own_m = cas.retained_judgment_bytes() + 64;
            assert_eq!(own_m, m);
            let law = heap
                .prepare_retained_expm1(
                    cas,
                    &original,
                    [-2.0, 0.0],
                    Policy::default(),
                    &ArithmeticContext::new()?,
                    &mut Counts::default(),
                    &mut account,
                )
                .map_err(|(error, _)| error)?;
            if spare {
                let extra = heap.retain_retained_expm1(&law, &mut account)?;
                drop(extra);
                assert!(matches!(
                    heap.reconcile_detached(&mut account, 1000, None),
                    crate::ScanOutcome::Complete
                ));
            }
            // The new owner is frozen; an unrelated warm preparation provides the persistent
            // registration. The law's own registration remains live and is counted separately.
            other.prepare_expm1(
                &request,
                Policy::default(),
                &mut account.ledger,
                &mut Counts::default(),
            )?;
            let before = heap.resource_bytes()?;
            let identity = account.identity;
            let injection = Injection::after(usize::MAX);
            let result = account.ledger.with_retained_payload(PREFIX, AT, |budget| {
                super::super::retention::acquire(&mut heap, &law, identity, budget)
            });
            if short == 0 {
                let extra = result?;
                assert_eq!(Injection::attempts(), 1 + usize::from(!spare));
                assert_eq!(
                    heap.resource_bytes()? - before,
                    u + usize::from(!spare) * row
                );
                assert!(std::ptr::eq(extra.authority()?, law.authority()?));
                drop(extra);
            } else {
                let LoanError::Budget(fault) = result.expect_err("use staged boundary") else {
                    return Err("typed use refusal".into());
                };
                assert_eq!(
                    fault.kind,
                    FaultKind::Exhausted {
                        reason: Exhaustion::Limit,
                        budget: BudgetKind::StagedPayloadBytes,
                        limit: own_m + m + u64::try_from(staged)? - 1,
                        needed: own_m + m + u64::try_from(staged)?
                    }
                );
                assert_eq!(Injection::attempts(), 0);
                assert_eq!(heap.resource_bytes()?, before);
            }
            drop(injection);
        }
    }
    Ok(())
}
