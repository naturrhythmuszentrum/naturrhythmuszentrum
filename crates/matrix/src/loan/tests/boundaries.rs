// SPDX-License-Identifier: AGPL-3.0-only
//! Independent layout arithmetic pins original-account boundaries without calling the estimator.

use super::{TestResult, fixture, granted};
use crate::heap::{descriptors::DescriptorRecord, identity::Key};
use crate::loan::registry::{Debit, Liveness, PinIndex, PinRecord};
use crate::loan::{CancelOutcome, LoanError, LoanRequest};
use crate::mask::storage::{LayoutRecord, MaskRecord, Native, Store};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// The independently laid-out occupied/vacant slot has one generation and one optional value.
#[repr(C)]
struct SlotMirror<T> {
    /// Current occupation number, including exhausted vacancies.
    generation: u64,
    /// Full tagged record header, independent of whether a payload is present.
    value: Option<T>,
}
/// Aligned shared allocation accounting follows the public design's C-layout boundary.
#[repr(C)]
struct SharedMirror<T> {
    /// Strong handle counter.
    strong: usize,
    /// Weak handle counter.
    weak: usize,
    /// Complete inline payload.
    value: T,
}
/// Literal owner-layout sum for a fresh heap with a single identity declaration.
fn bytes<P: Scalar>(cells: usize, components: usize, channels: usize) -> [usize; 2] {
    let entries = cells * components;
    let payload = entries.div_ceil(64 / size_of::<P>()) * 64;
    let flags = 8 * (entries.div_ceil(64) + cells.div_ceil(64));
    let debits = 3 * channels + 5;
    let retained = channels * (payload + flags)
        + crate::loan::abi::host_header()
        + size_of::<SharedMirror<LayoutRecord>>()
        + size_of::<SharedMirror<Liveness>>()
        + crate::loan::abi::bundle_header()
        + size_of::<SharedMirror<PinRecord>>()
        + channels * size_of::<SlotMirror<DescriptorRecord>>()
        + size_of::<SlotMirror<Native<LayoutRecord>>>()
        + size_of::<SlotMirror<Native<MaskRecord>>>()
        + channels * crate::loan::abi::group_row()
        + debits * size_of::<Option<Debit>>()
        + size_of::<Option<PinIndex>>();
    let private = debits * size_of::<Debit>()
        + (channels + 2) * size_of::<Key>()
        + channels * size_of::<DescriptorRecord>()
        + size_of::<Native<LayoutRecord>>()
        + size_of::<Native<MaskRecord>>();
    [retained, retained + private]
}
/// Both ceiling channels stop at the immediate insufficient neighbor, including row rounding.
fn byte_boundaries<P: Scalar + 'static>() -> TestResult {
    for cells in [0_usize, 1, 8, 9, 16, 17, 65] {
        for channels in 1..=3 {
            let [retained, staged] = bytes::<P>(cells, 2, channels);
            for staging in [false, true] {
                let required = if staging { staged } else { retained };
                for (limit, success) in [(required, true), (required - 1, false)] {
                    let mut limits = Limits::default();
                    if staging {
                        limits.staged_payload_bytes = u64::try_from(limit)?;
                    } else {
                        limits.output_bytes = u64::try_from(limit)?;
                    }
                    let (mut heap, mut account, declaration) =
                        fixture::<P>(cells, 2, [channels > 1, channels > 2], limits)?;
                    let request = heap.request_loan(declaration, &mut account);
                    if success {
                        let loan = granted(request)?;
                        assert_eq!(heap.resource_bytes()?, retained + size_of::<Store>());
                        assert!(matches!(
                            heap.cancel_loan(loan, &mut account),
                            CancelOutcome::Cancelled(_)
                        ));
                    } else {
                        assert!(
                            matches!(request, LoanRequest::Refused(LoanError::Heap(crate::HeapError::ResourceBytes { staged: actual, .. })) if actual == staging)
                        );
                        assert_eq!(heap.loans.elements()?, [0, 0]);
                        assert_eq!(heap.descriptors.slots.iter().count(), 0);
                    }
                }
            }
        }
    }
    Ok(())
}
/// All closed host representations pay their exact detached header and simultaneous buffer peak.
/// ´test:unit:loan-byte-boundaries-all-representations-and-row-rounding´
#[test]
fn loan_byte_boundaries_all_representations_and_row_rounding() -> TestResult {
    byte_boundaries::<f32>()?;
    byte_boundaries::<f64>()
}
/// Independently sum fixed cleanup, record acquisition, scalar initialization and word steps.
fn work_boundaries<P: Scalar + 'static>() -> TestResult {
    for cells in [0_usize, 1, 65] {
        for channels in 1..=3 {
            let entries = cells * 2;
            // Prior 3; declaration h; authentication 1; fresh allocations 18+3h;
            // records 4h+9 twice; non-pin edges h+3 twice; cancellation auth 4h+9, terminal/drop 2;
            // initial pin creation 1; scalar and packed-word initialization h(L+q+r).
            let required = 58
                + 15 * channels
                + channels
                    * (3 * usize::from(cells != 0)
                        + entries
                        + entries.div_ceil(64)
                        + cells.div_ceil(64));
            for (limit, success) in [(required, true), (required - 1, false)] {
                let (mut heap, mut account, declaration) = fixture::<P>(
                    cells,
                    2,
                    [channels > 1, channels > 2],
                    Limits {
                        work: u64::try_from(limit)?,
                        ..Limits::default()
                    },
                )?;
                let request = heap.request_loan(declaration, &mut account);
                if success {
                    let loan = granted(request)?;
                    assert_eq!(account.usage().work, u64::try_from(required)?);
                    assert!(matches!(
                        heap.cancel_loan(loan, &mut account),
                        CancelOutcome::Cancelled(_)
                    ));
                    assert_eq!(account.usage().work, u64::try_from(required)?);
                } else {
                    assert!(matches!(
                        request,
                        LoanRequest::Refused(LoanError::Budget(_))
                    ));
                    assert_eq!(heap.loans.elements()?, [0, 0]);
                }
            }
        }
    }
    Ok(())
}
/// Exact Work with prior cumulative usage admits N and refuses N minus one without a producer.
/// ´test:unit:loan-work-boundaries-all-representations´
#[test]
fn loan_work_boundaries_all_representations() -> TestResult {
    work_boundaries::<f32>()?;
    work_boundaries::<f64>()
}

/// Filled validation, flag derivation and record preparation have a literal complete-return boundary.
fn handback_work<P: Scalar + 'static>() -> TestResult {
    for channels in 1..=3 {
        // Admission with prior usage: 58+21h; writes h; return authentication 4h+9;
        // debit search 3h+5; inventory 6h+12; cleanup 7; zero word 1;
        // completion 2h; intersection h+1; normalization 1; payload h;
        // declaration/layout/private-shell revalidation h+5; publication records h+6.
        let required = 105 + 41 * channels;
        for (limit, success) in [(required, true), (required - 1, false)] {
            let (mut heap, mut account, declaration) = fixture::<P>(
                1,
                1,
                [channels > 1, channels > 2],
                Limits {
                    work: u64::try_from(limit)?,
                    ..Limits::default()
                },
            )?;
            let mut loan = granted(heap.request_loan(declaration, &mut account))?;
            for channel in crate::loan::HostChannel::ALL.into_iter().take(channels) {
                loan.write_cell(channel, 0, &[P::ONE], &mut account)?;
            }
            let outcome = heap.handback(loan, &mut account);
            match outcome {
                crate::HandbackOutcome::Published(tile, _) => {
                    assert!(success);
                    assert_eq!(account.usage().work, u64::try_from(required)?);
                    drop(tile);
                }
                crate::HandbackOutcome::Repair(LoanError::Budget(_), loan) => {
                    assert!(!success);
                    let before = account.usage().work;
                    assert!(matches!(
                        heap.cancel_loan(loan, &mut account),
                        CancelOutcome::Cancelled(_)
                    ));
                    assert_eq!(account.usage().work, before);
                }
                other => return Err(format!("{other:?}").into()),
            }
            let mut scanner =
                crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
            super::empty(&mut heap, &mut scanner)?;
        }
    }
    Ok(())
}
/// All representations return at N, repair at N minus one, and retain prepaid cancellation.
/// ´test:unit:loan-handback-work-boundaries-with-prior-usage´
#[test]
fn loan_handback_work_boundaries_with_prior_usage() -> TestResult {
    handback_work::<f32>()?;
    handback_work::<f64>()
}

/// A new account pays only its own pin header and complete replacement table, never payload copies.
/// ´test:unit:loan-pin-byte-boundaries-and-shared-payload-inventory´
#[test]
fn loan_pin_byte_boundaries_and_shared_payload_inventory() -> TestResult {
    let retained = size_of::<SharedMirror<PinRecord>>() + size_of::<Option<PinIndex>>();
    let staged = retained + 2 * size_of::<Option<PinIndex>>();
    for staging in [false, true] {
        let required = if staging { staged } else { retained };
        for (limit, success) in [(required, true), (required - 1, false)] {
            let (mut heap, mut account, declaration) =
                fixture::<f64>(1, 1, [true, true], Limits::default())?;
            let mut loan = granted(heap.request_loan(declaration, &mut account))?;
            let ticket = loan.ticket();
            let out = heap.loan_inventory(ticket, &mut account)?;
            assert_eq!(out.numerical_bytes, 192);
            assert_eq!(out.initialization_bytes, 48);
            assert_eq!(out.filled_bytes, 0);
            for channel in crate::loan::HostChannel::ALL {
                loan.write_cell(channel, 0, &[1.0], &mut account)?;
            }
            let tile = match heap.handback(loan, &mut account) {
                crate::HandbackOutcome::Published(tile, _) => tile,
                other => return Err(format!("{other:?}").into()),
            };
            let before = heap.loan_inventory(ticket, &mut account)?;
            assert_eq!(before.initialization_bytes, 0);
            assert_eq!(
                before.filled_bytes,
                size_of::<SharedMirror<MaskRecord>>() + 8
            );
            let mut limits = Limits::default();
            if staging {
                limits.staged_payload_bytes = u64::try_from(limit)?;
            } else {
                limits.output_bytes = u64::try_from(limit)?;
            }
            let mut reader_account =
                crate::ExecutionAccount::new(nrz_bindings::Budget::new(limits)?)?;
            let pin = heap.acquire_pin::<f64>(ticket, &mut reader_account);
            assert_eq!(pin.is_ok(), success);
            if let Ok(pin) = pin {
                let after = heap.loan_inventory(ticket, &mut account)?;
                assert_eq!(after.numerical_bytes, before.numerical_bytes);
                assert_eq!(after.filled_bytes, before.filled_bytes);
                assert_eq!(after.owner_bytes, before.owner_bytes);
                assert_eq!(
                    after.pin_bytes - before.pin_bytes,
                    size_of::<SharedMirror<PinRecord>>()
                );
                drop(pin);
            }
            drop(tile);
            super::empty(&mut heap, &mut account)?;
        }
    }
    Ok(())
}

/// Intervening independently funded owners can make publication's table replacement exceed its account.
/// ´test:unit:loan-late-byte-refusal-preserves-owner-for-zero-work-cancel´
#[test]
fn loan_late_byte_refusal_preserves_owner_for_zero_work_cancel() -> TestResult {
    let [_, initial_peak] = bytes::<f64>(1, 1, 1);
    let (mut heap, mut account, declaration) = fixture::<f64>(
        1,
        1,
        [false, false],
        Limits {
            staged_payload_bytes: u64::try_from(initial_peak)?,
            ..Limits::default()
        },
    )?;
    let symbols = declaration.symbols().clone();
    let axis = declaration.axis;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    loan.write_cell(crate::HostChannel::Center, 0, &[17.0], &mut account)?;
    let mut other = crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
    let mut loans = Vec::new();
    for _ in 0..16 {
        let declaration = crate::HostTileDeclaration::<f64>::check(
            symbols.clone(),
            axis,
            crate::Coverage::RepresentedInputs,
            &mut other,
        )?;
        loans.push(granted(heap.request_loan(declaration, &mut other))?);
    }
    let loan = match heap.handback(loan, &mut account) {
        crate::HandbackOutcome::Repair(
            LoanError::Heap(crate::HeapError::ResourceBytes { staged: true, .. }),
            loan,
        ) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    let remaining = account.limits().work - account.usage().work;
    account.ledger.work(remaining, crate::loan::AT)?;
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    for loan in loans {
        assert!(matches!(
            heap.cancel_loan(loan, &mut other),
            CancelOutcome::Cancelled(_)
        ));
    }
    super::empty(&mut heap, &mut other)
}

/// Clearing one 65-component cell touches two initialization words and one complete-cell bit.
/// ´test:unit:loan-clear-word-boundary-is-atomic-at-insufficient-work´
#[test]
fn loan_clear_word_boundary_is_atomic_at_insufficient_work() -> TestResult {
    for remaining in [3_u64, 2] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(1, 65, [false, false], Limits::default())?;
        let mut loan = granted(heap.request_loan(declaration, &mut account))?;
        loan.write_cell(crate::HostChannel::Center, 0, &[1.0; 65], &mut account)?;
        let spend = account.limits().work - account.usage().work - remaining;
        account.ledger.work(spend, crate::loan::AT)?;
        let before = account.usage().work;
        let result = loan.clear_cell(crate::HostChannel::Center, 0, &mut account);
        let flags = &loan.core.channels[0].as_ref().ok_or("center")?.components;
        if remaining == 3 {
            result?;
            assert_eq!(flags.as_slice(), [0, 0]);
            assert_eq!(account.usage().work - before, 3);
        } else {
            assert!(matches!(result, Err(LoanError::Budget(_))));
            assert_eq!(flags.as_slice(), [u64::MAX, 1]);
            assert_eq!(account.usage().work, before);
        }
        assert!(matches!(
            heap.cancel_loan(loan, &mut account),
            CancelOutcome::Cancelled(_)
        ));
    }
    Ok(())
}

/// Root removal requires a further unchanged pass, with every vacancy charged again.
/// ´test:unit:loan-reclaim-pass-work-boundaries´
#[test]
fn loan_reclaim_pass_work_boundaries() -> TestResult {
    for channels in 1..=3 {
        let required = 4 * channels + 10;
        for (remaining, success) in [(required, true), (required - 1, false)] {
            let (mut heap, mut account, declaration) =
                fixture::<f64>(1, 1, [channels > 1, channels > 2], Limits::default())?;
            let loan = granted(heap.request_loan(declaration, &mut account))?;
            let tile = match heap.handback(loan, &mut account) {
                crate::HandbackOutcome::Published(tile, _) => tile,
                other => return Err(format!("{other:?}").into()),
            };
            drop(tile);
            assert!(matches!(
                heap.reconcile_detached(&mut account, 1000, None),
                crate::ScanOutcome::Complete
            ));
            let spend = account.limits().work - account.usage().work - u64::try_from(remaining)?;
            account.ledger.work(spend, crate::loan::AT)?;
            let before = account.usage().work;
            let result = heap.reclaim(&mut account, 1000, None);
            if success {
                assert!(matches!(result, crate::ScanOutcome::Complete));
            } else {
                assert!(matches!(result, crate::ScanOutcome::WorkExhausted(_)));
            }
            assert_eq!(account.usage().work - before, u64::try_from(remaining)?);
            assert!(matches!(
                heap.residence(&mut account, 1000)?,
                crate::Residence::Pending { .. }
            ));
            let mut scanner =
                crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
            super::empty(&mut heap, &mut scanner)?;
        }
    }
    Ok(())
}

/// Independent pin creation stops before allocation when its last prepaid step cannot be funded.
/// ´test:unit:loan-pin-work-boundary-preserves-original-edge´
#[test]
fn loan_pin_work_boundary_preserves_original_edge() -> TestResult {
    for (remaining, success) in [(54_u64, true), (53, false)] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(1, 1, [false, false], Limits::default())?;
        let loan = granted(heap.request_loan(declaration, &mut account))?;
        let tile = match heap.handback(loan, &mut account) {
            crate::HandbackOutcome::Published(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        let spend = account.limits().work - account.usage().work - remaining;
        account.ledger.work(spend, crate::loan::AT)?;
        let before = account.usage().work;
        let result = heap.retain_pin(tile.pin(), &mut account);
        if success {
            let pin = result?;
            assert_eq!(account.usage().work - before, 54);
            assert!(heap.release_pin(pin, &mut account).is_ok());
            assert_eq!(account.usage().work - before, 54);
        } else {
            assert!(matches!(result, Err(LoanError::Budget(_))));
            assert_eq!(account.usage().work - before, 45);
        }
        drop(tile);
        let mut scanner =
            crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
        super::empty(&mut heap, &mut scanner)?;
    }
    Ok(())
}

/// Current access pays repeated immutable checks before exposing its borrowed selected reader.
/// ´test:unit:loan-current-reader-work-boundary´
#[test]
fn loan_current_reader_work_boundary() -> TestResult {
    for (remaining, success) in [(18_u64, true), (17, false)] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(1, 1, [false, false], Limits::default())?;
        let loan = granted(heap.request_loan(declaration, &mut account))?;
        let tile = match heap.handback(loan, &mut account) {
            crate::HandbackOutcome::Published(tile, _) => tile,
            other => return Err(format!("{other:?}").into()),
        };
        let spend = account.limits().work - account.usage().work - remaining;
        account.ledger.work(spend, crate::loan::AT)?;
        let before = account.usage().work;
        assert_eq!(tile.reader(&heap, &mut account).is_ok(), success);
        assert_eq!(account.usage().work - before, if success { 18 } else { 15 });
        drop(tile);
        let mut scanner =
            crate::ExecutionAccount::new(nrz_bindings::Budget::new(Limits::default())?)?;
        super::empty(&mut heap, &mut scanner)?;
    }
    Ok(())
}

/// Column-major strides below, at and above one word preserve the distinct-word tariff.
/// ´test:unit:loan-clear-stride-word-tariff-precedes-mutation´
#[test]
fn loan_clear_stride_word_tariff_precedes_mutation() -> TestResult {
    for capacity in [1, 31, 63, 64, 65] {
        let components = 65;
        let cell = capacity - 1;
        let words: std::collections::BTreeSet<_> = (0..components)
            .map(|component| (cell + capacity * component) / 64)
            .collect();
        let needed = u64::try_from(words.len() + 1)?;
        for (remaining, succeeds) in [(needed, true), (needed - 1, false)] {
            let (mut heap, mut account, declaration) =
                fixture::<f64>(capacity, components, [false, false], Limits::default())?;
            let mut loan = granted(heap.request_loan(declaration, &mut account))?;
            loan.write_cell(crate::HostChannel::Center, cell, &[1.0; 65], &mut account)?;
            let before = loan.core.channels[0]
                .as_ref()
                .ok_or("center")?
                .components
                .clone();
            let spend = account.limits().work - account.usage().work - remaining;
            account.ledger.work(spend, crate::loan::AT)?;
            let used = account.usage().work;
            let result = loan.clear_cell(crate::HostChannel::Center, cell, &mut account);
            let flags = &loan.core.channels[0].as_ref().ok_or("center")?.components;
            if succeeds {
                result?;
                assert!(flags.iter().all(|word| *word == 0));
                assert_eq!(account.usage().work - used, needed);
            } else {
                assert!(matches!(result, Err(LoanError::Budget(_))));
                assert_eq!(flags, &before);
                assert_eq!(account.usage().work, used);
            }
        }
    }
    Ok(())
}
