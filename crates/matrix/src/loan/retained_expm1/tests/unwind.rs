// SPDX-License-Identifier: AGPL-3.0-only
//! Scoped fault hooks cover identity and allocator unwinding without affecting parallel fixtures.

use super::{TestResult, fixture};
use crate::{Heap, HeapLimits, LoanError, RetainedExpm1Error};
use nrz_bindings::readings::algebra::enclosure::prepared_expm1::{Counts, Policy};
use nrz_scalar::ArithmeticContext;
use std::cell::Cell;

thread_local! {
    /// Only this thread's private law identities are constrained by a diagnostic.
    static IDENTITIES: Cell<Option<usize>> = const { Cell::new(None) };
}
/// The owner reuses the existing nonwrapping allocator; this hook isolates its refusal prefixes.
pub(super) fn before_identity() -> Result<(), LoanError> {
    if let Some(remaining) = IDENTITIES.get() {
        if remaining == 0 {
            return Err(crate::HeapError::CounterExhausted.into());
        }
        IDENTITIES.set(Some(remaining - 1));
    }
    Ok(())
}
/// Reset the thread-local identity allowance on both ordinary and unwinding paths.
struct IdentityGuard;
impl Drop for IdentityGuard {
    fn drop(&mut self) {
        IDENTITIES.set(None);
    }
}

/// Owner, initial use and receipt exhaustion precede every managed allocation or installed debit.
#[test]
fn retained_law_identity_exhaustion_prefixes() -> TestResult {
    for prefix in 0..3 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let (cas, request) = fixture::request()?;
        IDENTITIES.set(Some(prefix));
        let guard = IdentityGuard;
        let allocations = crate::loan::fault::Injection::after(usize::MAX);
        let (error, returned) = heap
            .prepare_retained_expm1(
                cas,
                &request,
                [-2.0, 0.0],
                Policy::default(),
                &ArithmeticContext::new()?,
                &mut Counts::default(),
                &mut account,
            )
            .expect_err("identity exhaustion");
        assert!(matches!(
            error,
            RetainedExpm1Error::Ownership(LoanError::Heap(crate::HeapError::CounterExhausted))
        ));
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(heap.loans.debits.len(), 0);
        returned.expression(&request.original.target)?;
        drop((guard, allocations));
    }
    let mut heap = Heap::new(HeapLimits::default())?;
    let mut account = fixture::account()?;
    let law = fixture::prepare(&mut heap, &mut account)?;
    for prefix in 0..2 {
        IDENTITIES.set(Some(prefix));
        let guard = IdentityGuard;
        assert!(matches!(
            heap.retain_retained_expm1(&law, &mut account),
            Err(LoanError::Heap(crate::HeapError::CounterExhausted))
        ));
        drop(guard);
    }
    assert_eq!(heap.retained_expm1_inventory(&law, &mut account)?.uses, 1);
    Ok(())
}
/// Every acquired private prefix unwinds under its original prepaid cleanup entitlement.
#[test]
fn retained_law_preparation_unwind_prefixes() -> TestResult {
    for prefix in 0..3 {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let (cas, request) = fixture::request()?;
        let context = ArithmeticContext::new()?;
        let injection = crate::loan::fault::Injection::unwinding_after(prefix);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            drop(heap.prepare_retained_expm1(
                cas,
                &request,
                [-2.0, 0.0],
                Policy::default(),
                &context,
                &mut Counts::default(),
                &mut account,
            ));
        }));
        assert!(result.is_err());
        let retained = usize::from(prefix != 0);
        assert_eq!(
            crate::loan::fault::Injection::prefix(),
            [prefix, retained, prefix - retained]
        );
        assert_eq!(heap.loans.debits.iter().flatten().count(), 0);
        drop(injection);
    }
    Ok(())
}

/// Failed extra uses preserve the original authority through every acquired allocation prefix.
#[test]
fn retained_law_extra_use_cleanup_prefixes() -> TestResult {
    for unwind in [false, true] {
        for prefix in 0..2 {
            let mut heap = Heap::new(HeapLimits::default())?;
            let mut account = fixture::account()?;
            let law = fixture::prepare(&mut heap, &mut account)?;
            let before = heap.retained_expm1_inventory(&law, &mut account)?;
            let destroyed = super::DESTROYED.get();
            let injection = if unwind {
                crate::loan::fault::Injection::unwinding_after(prefix)
            } else {
                crate::loan::fault::Injection::after(prefix)
            };
            if unwind {
                assert!(
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drop(heap.retain_retained_expm1(&law, &mut account));
                    }))
                    .is_err()
                );
            } else {
                assert!(matches!(
                    heap.retain_retained_expm1(&law, &mut account),
                    Err(LoanError::AllocationRefused)
                ));
            }
            assert_eq!(crate::loan::fault::Injection::prefix(), [prefix, prefix, 0]);
            assert_eq!(heap.loans.debits.iter().flatten().count(), 2);
            assert_eq!(heap.loans.debits.capacity(), 2 + prefix);
            drop(injection);
            assert_eq!(heap.retained_expm1_inventory(&law, &mut account)?, before);
            law.evaluate(fixture::point(-0.5), &ArithmeticContext::new()?)?;
            assert_eq!(super::DESTROYED.get(), destroyed);
            drop(law);
            assert_eq!(super::DESTROYED.get(), destroyed + 1);
        }
    }
    Ok(())
}

/// Unused owners and independent extra uses need neither a live heap nor fresh cleanup Work.
#[test]
fn retained_law_unused_and_extra_use_unwind() -> TestResult {
    for extra in [false, true] {
        let mut heap = Heap::new(HeapLimits::default())?;
        let mut account = fixture::account()?;
        let law = fixture::prepare(&mut heap, &mut account)?;
        let second = if extra {
            Some(heap.retain_retained_expm1(&law, &mut account)?)
        } else {
            None
        };
        let destroyed = super::DESTROYED.get();
        let work = account.usage().work;
        drop(heap);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _uses = (law, second);
                panic!("last law use unwind fixture");
            }))
            .is_err()
        );
        assert_eq!(super::DESTROYED.get(), destroyed + 1);
        assert_eq!(account.usage().work, work);
    }
    Ok(())
}
