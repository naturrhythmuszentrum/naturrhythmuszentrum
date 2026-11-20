// SPDX-License-Identifier: AGPL-3.0-only
//! Hand-enumerated boundaries include a used ledger, old storage and failed acquisition prefixes.

use super::super::{Arguments, Handle, Inventory, Reservation};
use super::{BYTES, TestResult, WORK, account, args};
use crate::{LoanError, loan::fault::Injection};
use nrz_bindings::{
    Limits,
    readings::algebra::{FaultKind, limits::BudgetKind},
};
use nrz_scalar::Scalar;
use std::sync::atomic::AtomicU64;

/// One top-level focused control runs both formats through all these cases.
pub(super) fn run<P: Scalar + 'static>() -> TestResult {
    for staged in [false, true] {
        bytes::<P>(staged)?;
    }
    work::<P>()?;
    handles::<P>()?;
    capacities::<P>()?;
    prefixes::<P>()
}
/// 832 exact bytes and 831 refuse; two distinct retained owners need 1,664 bytes.
fn bytes<P: Scalar + 'static>(staged: bool) -> TestResult {
    for count in [1_u64, 2] {
        for short in [0, 1] {
            let boundary = 832 * count - short;
            let limits = if staged {
                Limits {
                    staged_payload_bytes: boundary,
                    ..Limits::default()
                }
            } else {
                Limits {
                    payload_bytes: boundary,
                    ..Limits::default()
                }
            };
            let mut account = account(limits)?;
            let old = if count == 2 {
                Some(Reservation::reserve::<P>(args(), &[], &mut account)?)
            } else {
                None
            };
            let prior: Vec<_> = old.iter().map(Reservation::borrowed).collect();
            let before = account.usage().work;
            let outcome = Reservation::reserve::<P>(args(), &prior, &mut account);
            if short == 0 {
                let owner = outcome?;
                assert_eq!(
                    Inventory::inspect(&[owner.borrowed()], &mut account)?.covered_bytes,
                    BYTES
                );
            } else {
                let error = outcome.expect_err("one below the independently enumerated peak");
                let kind = if staged {
                    BudgetKind::StagedPayloadBytes
                } else {
                    BudgetKind::PayloadBytes
                };
                assert!(
                    matches!(error, LoanError::Budget(fault) if matches!(fault.kind,
                    FaultKind::Exhausted { budget, limit, needed, .. } if budget == kind && limit == boundary && needed == 832 * count))
                );
                // Empty scope: H=1. One old owner: H scope entry + H backing entry=2. Two argument records follow.
                assert_eq!(
                    account.usage().work - before,
                    if count == 1 { 3 } else { 4 }
                );
            }
            if let Some(old) = old {
                assert_eq!(
                    Inventory::inspect(&[old.borrowed()], &mut account)?.covered_bytes,
                    BYTES
                );
            }
        }
    }
    Ok(())
}
/// Five previous Work plus 31 acquisition Work succeeds; 35 refuses before acquisition.
fn work<P: Scalar + 'static>() -> TestResult {
    for short in [0, 1] {
        let mut account = account(Limits {
            work: 5 + WORK - short,
            ..Limits::default()
        })?;
        let result = Reservation::reserve::<P>(args(), &[], &mut account);
        if short == 0 {
            let owner = result?;
            assert_eq!(account.usage().work, 36);
            assert_eq!(owner.backing.nodes[0].attempts, 100);
            drop(owner);
            assert_eq!(account.usage().work, 36);
        } else {
            assert!(matches!(result, Err(LoanError::Budget(_))));
            assert_eq!(account.usage().work, 8);
        }
    }
    Ok(())
}
/// A foreign or stale inspection never consumes the actual reservation or its original handle.
fn handles<P: Scalar + 'static>() -> TestResult {
    let mut account = account(Limits::default())?;
    let owner = Reservation::reserve::<P>(args(), &[], &mut account)?;
    let handle = owner.handle(0, &mut account)?;
    let mut foreign = super::account(Limits::default())?;
    let before = foreign.usage().work;
    assert!(matches!(
        owner.inspect(handle, &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    assert_eq!(foreign.usage().work, before);
    assert!(matches!(
        Inventory::inspect(&[owner.borrowed()], &mut foreign),
        Err(LoanError::ForeignAccount)
    ));
    let other = Reservation::reserve::<P>(args(), &[owner.borrowed()], &mut account)?;
    assert!(matches!(
        other.inspect(handle, &mut account),
        Err(LoanError::ForeignOwner)
    ));
    let stale = Handle {
        generation: handle.generation + 1,
        ..handle
    };
    assert!(matches!(
        owner.inspect(stale, &mut account),
        Err(LoanError::StaleOccupation)
    ));
    let absent = Handle { slot: 2, ..handle };
    assert!(matches!(
        owner.inspect(absent, &mut account),
        Err(LoanError::StaleOccupation)
    ));
    assert_eq!(owner.inspect(handle, &mut account)?.attempts, 100);
    Ok(())
}
/// Reserved vacancies, zero graph storage and checked caller count refusals are separate cases.
fn capacities<P: Scalar + 'static>() -> TestResult {
    let mut account = account(Limits::default())?;
    let small = Arguments { nodes: 1, ..args() };
    assert!(matches!(
        Reservation::reserve::<P>(small, &[], &mut account),
        Err(LoanError::UnsupportedContract)
    ));
    let spare = Arguments { edges: 2, ..args() };
    assert!(matches!(
        Reservation::reserve::<P>(spare, &[], &mut account),
        Err(LoanError::UnsupportedContract)
    ));
    let overflow = Arguments {
        attempts: &[u64::MAX, 1],
        ..args()
    };
    assert!(matches!(
        Reservation::reserve::<P>(overflow, &[], &mut account),
        Err(LoanError::CountOverflow)
    ));
    let huge = Arguments {
        nodes: usize::MAX,
        ..args()
    };
    assert!(matches!(
        Reservation::reserve::<P>(huge, &[], &mut account),
        Err(LoanError::ByteOverflow)
    ));
    let vacant = Reservation::reserve::<P>(
        Arguments {
            attempts: &[9],
            ..args()
        },
        &[],
        &mut account,
    )?;
    assert_eq!(vacant.backing.nodes[1].generation, 0);
    assert_eq!(
        Inventory::inspect(&[vacant.borrowed()], &mut account)?.covered_bytes,
        832
    );
    assert!(matches!(
        vacant.handle(1, &mut account),
        Err(LoanError::WrongLoan)
    ));
    let zero = Arguments {
        nodes: 0,
        edges: 0,
        proposed_edges: 0,
        scopes: 0,
        drive: 0,
        attempts: &[],
    };
    let empty = Reservation::reserve::<P>(zero, &[vacant.borrowed()], &mut account)?;
    assert_eq!(empty.backing.ready, [] as [u64; 0]);
    assert_eq!(
        Inventory::inspect(&[empty.borrowed()], &mut account)?.covered_bytes,
        376
    );
    // Exercise the actual reservation path with an isolated identity source, not NEXT_ID.
    let exhausted = AtomicU64::new(u64::MAX);
    assert!(matches!(
        Reservation::reserve_with::<P>(args(), &[], &mut account, &exhausted),
        Err(LoanError::Heap(crate::HeapError::CounterExhausted))
    ));
    Ok(())
}
/// Each of eight backing boundaries leaves only its previously acquired lexical prefix to drop.
fn prefixes<P: Scalar + 'static>() -> TestResult {
    for acquired in 0..8 {
        let mut account = account(Limits::default())?;
        let old = Reservation::reserve::<P>(args(), &[], &mut account)?;
        let injection = Injection::after(acquired);
        let result = Reservation::reserve::<P>(args(), &[old.borrowed()], &mut account);
        assert!(matches!(result, Err(LoanError::AllocationRefused)));
        assert_eq!(Injection::prefix(), [acquired, 0, acquired]);
        // Prior 5 + old 31 + new 32 (one extra H to inspect the old backing).
        // Every allocation failure retains the entire prepaid event/cleanup inventory.
        assert_eq!(account.usage().work, 68);
        drop(injection);
        assert_eq!(
            Inventory::inspect(&[old.borrowed()], &mut account)?.covered_bytes,
            BYTES
        );
    }
    Ok(())
}
