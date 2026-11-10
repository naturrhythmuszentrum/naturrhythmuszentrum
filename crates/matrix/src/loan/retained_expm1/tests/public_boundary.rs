// SPDX-License-Identifier: AGPL-3.0-only
//! A public constructor's independently enumerated replacement peak dominates earlier exact preparation.
//! Graph G and judgment J are public CAS inventories: these are ledger-composition
//! controls, not independent CAS inventory controls. Omitting a CAS inventory item
//! would lower both the expected ceiling and reservation. A separate whole-graph
//! census would duplicate private vertex payload, interning-key and proof encodings.

use super::{TestResult, bytes, fixture};
use crate::{ExecutionAccount, Heap, HeapLimits, LoanError, RetainedExpm1Error};
use nrz_bindings::{
    Budget, Limits,
    readings::algebra::{
        Cas, Exhaustion, FaultKind,
        enclosure::prepared_expm1::{Counts, Policy, Request},
        limits::BudgetKind,
    },
};
use nrz_scalar::ArithmeticContext;

/// Real unrelated uses occupy the debit table; no synthetic occupation or account reset is needed.
#[test]
fn retained_law_public_constructor_ledger_composition_boundary() -> TestResult {
    let (mut producing, request) = fixture::request()?;
    let context = ArithmeticContext::new()?;
    let parts = crate::prepared_expm1::owned_parts(
        &mut producing,
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
    // A seed owner and 1023 extra live uses occupy 1025 rows. The next owner adds
    // two rows and simultaneously replaces all 1025 old rows; there are no T backings.
    let occupied = 1025;
    let managed = header + evidence + use_header + 2 * row + 2 * occupied * row;
    let graph = producing.counts().payload_bytes;
    let registration = producing.retained_judgment_bytes() + 64;
    // Both owner constructions are warm, so the first owner's unrelated registration
    // and the producing graph's registration metadata remain additive.
    let boundary = graph + 64 + registration + u64::try_from(managed)?;
    drop((parts, producing));
    for short in [0, 1] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = ExecutionAccount::new(Budget::new(Limits {
            staged_payload_bytes: boundary - short,
            ..fixture::limits()
        })?)?;
        let (seed, seed_request) = warm()?;
        let first = heap
            .prepare_retained_expm1(
                seed,
                &seed_request,
                [-2.0, 0.0],
                Policy::default(),
                &context,
                &mut Counts::default(),
                &mut account,
            )
            .map_err(|(error, _)| error)?;
        let mut uses = Vec::new();
        for _ in 0..1023 {
            uses.push(heap.retain_retained_expm1(&first, &mut account)?);
        }
        assert_eq!(heap.loans.debits.capacity(), occupied);
        let (cas, request) = warm()?;
        assert_eq!(cas.counts().payload_bytes, graph);
        assert_eq!(cas.retained_judgment_bytes() + 64, registration);
        let before = heap.resource_bytes()?;
        let injection = crate::loan::fault::Injection::after(usize::MAX);
        let result = heap.prepare_retained_expm1(
            cas,
            &request,
            [-2.0, 0.0],
            Policy::default(),
            &context,
            &mut Counts::default(),
            &mut account,
        );
        if short == 0 {
            let law = result.map_err(|(error, _)| error)?;
            assert_eq!(crate::loan::fault::Injection::attempts(), 3);
            assert_eq!(
                heap.resource_bytes()? - before,
                header + evidence + use_header + 2 * row
            );
            law.evaluate(fixture::point(0.0), &context)?;
        } else {
            let (error, returned) = result.expect_err("public combined boundary");
            let RetainedExpm1Error::Ownership(LoanError::Budget(fault)) = error else {
                return Err(format!("{error:?}").into());
            };
            assert_eq!(
                fault.kind,
                FaultKind::Exhausted {
                    reason: Exhaustion::Limit,
                    budget: BudgetKind::StagedPayloadBytes,
                    limit: 2 * registration + u64::try_from(managed)? - 1,
                    needed: 2 * registration + u64::try_from(managed)?
                }
            );
            returned.expression(&request.original.target)?;
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!(heap.resource_bytes()?, before);
        }
        drop(injection);
        assert_eq!(
            heap.retained_expm1_inventory(&first, &mut account)?.uses,
            1024
        );
        drop(uses);
    }
    Ok(())
}

/// Install real prepared evidence on a separate seed ledger before its checked registration.
fn warm() -> TestResult<(Cas, Request)> {
    let (mut cas, request) = fixture::request()?;
    cas.prepare_expm1(
        &request,
        Policy::default(),
        &mut Budget::new(fixture::limits())?,
        &mut Counts::default(),
    )?;
    Ok((cas, request))
}
