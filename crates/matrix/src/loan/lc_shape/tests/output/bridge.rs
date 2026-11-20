// SPDX-License-Identifier: AGPL-3.0-only
//! Original declarations survive consuming refusals and own detached captured pins.
mod keys;
mod oracle;
use super::{TestResult, request, setup};
use crate::loan::lc_shape::output::bridge::prepare_lc_omega_correspondence;
use crate::{ExecutionAccount, Heap, LcOmegaOutput, LoanError};
use nrz_bindings::{Budget, Limits, readings::algebra::limits::BudgetKind};
use nrz_scalar::Scalar;

/// Actual matrix originals construct and replay before any native facade can exist.
/// ´test:unit:lc-origin-correspondence-root-replay-boundaries´
#[test]
fn lc_origin_correspondence_root_replay_boundaries() -> TestResult {
    originals::<f32>()?;
    originals::<f64>()?;
    portable_refusal::<f32>()?;
    portable_refusal::<f64>()
}

fn originals<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let key = output.specialization_key();
    let before = s.account.usage().work;
    let root = prepare_lc_omega_correspondence(&output, &s.registry, None, &mut s.account)?;
    assert_eq!(
        s.account.usage().work - before,
        oracle::token_work() + oracle::work(false)
    );
    assert_eq!(root.retained_bytes()?, 424 + oracle::retained(false));
    assert_eq!(oracle::staged(false), 66_968);
    let before = s.account.usage().work;
    root.authenticate(&output, &s.registry, &mut s.account.ledger)?;
    // Three sealed visits plus fixed 5 and seven visits per original position.
    assert_eq!(s.account.usage().work - before, 3 + 5 + 7 * 5);
    let certificate = root.certificate(&mut s.account.ledger)?;
    oracle::wire(&certificate.to_bytes(&mut s.account.ledger)?)?;
    drop(root);
    let before = s.account.usage().work;
    let replay =
        prepare_lc_omega_correspondence(&output, &s.registry, Some(&certificate), &mut s.account)?;
    assert_eq!(
        s.account.usage().work - before,
        oracle::token_work() + oracle::work(true)
    );
    assert_eq!(replay.retained_bytes()?, 424 + oracle::retained(true));
    assert_eq!(oracle::staged(true), 77_310);
    let before = s.account.usage().work;
    replay.authenticate(&output, &s.registry, &mut s.account.ledger)?;
    assert_eq!(s.account.usage().work - before, 43);
    assert_eq!(output.specialization_key(), key);
    let mut foreign = ExecutionAccount::new(Budget::new(Limits::default())?)?;
    let before = foreign.usage().work;
    let refused = prepare_lc_omega_correspondence(&output, &s.registry, None, &mut foreign);
    let wrong_account = matches!(
        refused,
        Err(crate::LcShapeError::Loan(LoanError::ForeignAccount))
    );
    assert!(wrong_account);
    assert_eq!(foreign.usage().work, before);
    Ok(())
}

/// Borrowed agreement precedes a consuming join that retains independent pin edges.
/// ´test:unit:lc-omega-correspondence-join´
#[test]
fn lc_omega_correspondence_join() -> TestResult {
    borrowed::<f32>()?;
    borrowed::<f64>()?;
    consuming::<f32>()?;
    consuming::<f64>()?;
    allocation_refusals::<f32>()?;
    allocation_refusals::<f64>()?;
    native_refusals::<f32>()?;
    native_refusals::<f64>()?;
    defensive::<f32>()?;
    defensive::<f64>()?;
    revision_refusal::<f32>()?;
    revision_refusal::<f64>()?;
    paused_reconciliation::<f32>()?;
    paused_reconciliation::<f64>()
}

fn borrowed<P: Scalar + 'static>() -> TestResult {
    use crate::loan::lc_shape::output::bridge::borrowed::prepare_borrowed;
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let before = s.account.usage().work;
    let missing = prepare_borrowed(&output, &s.heap, &[], &s.registry, None, &mut s.account);
    let wrong_count = matches!(
        missing,
        Err(crate::LcShapeError::Loan(LoanError::UnsupportedContract))
    );
    assert!(wrong_count);
    assert_eq!(s.account.usage().work - before, 2);
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let tiles = members.references();
    crate::loan::fault::reset_reads();
    let injection = crate::loan::fault::Injection::after(0);
    let (candidate, _plan) =
        prepare_borrowed(&output, &s.heap, &tiles, &s.registry, None, &mut s.account)?;
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!(crate::loan::fault::reads(), 0);
    drop(injection);
    drop(candidate);
    drop(tiles);
    members.finish(&mut s.heap, &mut s.account)
}

fn consuming<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let key = output.specialization_key();
    let failure = output
        .prepare(&mut s.heap, &[], &s.registry, &mut s.account)
        .expect_err("missing native roles");
    let (output, error) = failure.into_parts();
    assert_eq!(output.specialization_key(), key);
    assert!(matches!(
        error,
        crate::LcShapeError::Loan(LoanError::UnsupportedContract)
    ));
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let tiles = members.references();
    let before = s.heap.loans.debits.iter().flatten().count();
    crate::loan::fault::reset_reads();
    let prepared = output
        .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
        .map_err(|failure| failure.into_parts().1)?;
    assert_eq!(crate::loan::fault::reads(), 0);
    assert_eq!(s.heap.loans.debits.iter().flatten().count(), before + 2);
    let witnesses: Vec<_> = tiles
        .iter()
        .map(|tile| std::rc::Rc::downgrade(&tile.pin().record))
        .collect();
    drop(tiles);
    let layouts = members.detach();
    assert!(witnesses.iter().all(|pin| pin.strong_count() == 1));
    assert!(matches!(
        s.heap.residence(&mut s.account, 100_000)?,
        crate::Residence::Pending { .. }
    ));
    assert!(witnesses.iter().all(|pin| pin.strong_count() == 1));
    let closure_debits = s
        .heap
        .loans
        .debits
        .iter()
        .flatten()
        .filter(|debit| {
            matches!(
                debit.kind,
                crate::loan::registry::AllocationKind::LcClosure(_)
                    | crate::loan::registry::AllocationKind::LcRows(_)
            )
        })
        .count();
    assert_eq!(closure_debits, 2);
    drop(prepared);
    assert!(witnesses.iter().all(|pin| pin.strong_count() == 0));
    for layout in layouts {
        super::fixture::retire_layout(&mut s.heap, &mut s.account, layout)?;
    }
    super::fixture::empty(&mut s.heap, &mut s.account)
}

fn allocation_refusals<P: Scalar + 'static>() -> TestResult {
    // Each native allocation prefix returns the same declaration, then remains retryable.
    for growing in [false, true] {
        for fail_after in 0..=3 + usize::from(growing) {
            let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
            let output = LcOmegaOutput::declare(
                &s.inventory,
                request(&s.registry, &s.radian),
                &mut s.account,
            )?;
            let key = output.specialization_key();
            let symbols = output.symbols().clone();
            let members = super::super::inventory::readers::publish(
                &s.inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                1,
                0,
            )?;
            let extra = if growing {
                Some(extra_loan(
                    &mut s.heap,
                    s.inventory.input(0).ok_or("first role")?,
                    &s.axes,
                    &mut s.account,
                )?)
            } else {
                None
            };
            let tiles = members.references();
            let debits = s.heap.loans.debits.iter().flatten().count();
            let before = s.account.usage().work;
            let injection = crate::loan::fault::Injection::after(fail_after);
            let result = output.prepare(&mut s.heap, &tiles, &s.registry, &mut s.account);
            drop(injection);
            match result {
                Ok(prepared) => {
                    // Spare-capacity fixtures have only the three payload allocations.
                    assert_eq!(fail_after, 3 + usize::from(growing));
                    drop(prepared);
                }
                Err(failure) => {
                    let (output, error) = failure.into_parts();
                    assert!(matches!(
                        error,
                        crate::LcShapeError::Loan(LoanError::AllocationRefused)
                    ));
                    assert_eq!(output.specialization_key(), key);
                    assert_eq!(output.symbols().value(), symbols.value());
                    assert_eq!(output.symbols().bound(), symbols.bound());
                    assert_eq!(output.symbols().error(), symbols.error());
                    assert!(std::ptr::eq(output.inventory(), &raw const s.inventory));
                    assert_eq!(s.heap.loans.debits.iter().flatten().count(), debits);
                    assert!(s.account.usage().work > before);
                    let prepared = output
                        .prepare(&mut s.heap, &tiles, &s.registry, &mut s.account)
                        .map_err(|failure| failure.into_parts().1)?;
                    drop(prepared);
                }
            }
            drop(tiles);
            if let Some(loan) = extra {
                assert!(matches!(
                    s.heap.cancel_loan(loan, &mut s.account),
                    crate::CancelOutcome::Cancelled(_)
                ));
            }
            members.finish(&mut s.heap, &mut s.account)?;
        }
    }
    Ok(())
}

fn native_refusals<P: Scalar + 'static>() -> TestResult {
    for case in 0..4 {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let symbols = output.symbols().clone();
        let key = output.specialization_key();
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            1,
            0,
        )?;
        let question = s.inventory.input(0).ok_or("first original")?.question();
        let foreign_inventory =
            crate::LcInputInventory::<P, 5>::declare(question, [2, 3, 2], &mut s.account)?;
        let foreign_members = if case == 3 {
            Some(super::super::inventory::readers::publish(
                &foreign_inventory,
                &s.axes,
                &mut s.heap,
                &mut s.account,
                1,
                0,
            )?)
        } else {
            None
        };
        let foreign_tiles = foreign_members.as_ref().map(|members| members.references());
        let mut tiles = members.references();
        if let Some(foreign) = &foreign_tiles {
            tiles[0] = foreign[0];
        }
        if case == 0 {
            tiles.swap(0, 4);
        }
        // A normal residence scan may reclaim the old host occupation while its
        // immutable mapped image remains pinned. A fresh join requires both current gates.
        if case == 2 {
            s.heap.residence(&mut s.account, 100_000)?;
        }
        let mut foreign = Heap::new(crate::HeapLimits::default())?;
        let heap = if case == 1 { &mut foreign } else { &mut s.heap };
        let injection = crate::loan::fault::Injection::after(0);
        let failure = output
            .prepare(heap, &tiles, &s.registry, &mut s.account)
            .expect_err("native authority mismatch");
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        drop(injection);
        let (output, error) = failure.into_parts();
        match case {
            0 | 3 => assert!(matches!(
                error,
                crate::LcShapeError::Loan(LoanError::TargetMismatch)
            )),
            1 => assert!(matches!(
                error,
                crate::LcShapeError::Loan(LoanError::ForeignOwner)
            )),
            _ => assert!(matches!(
                error,
                crate::LcShapeError::Loan(LoanError::StaleOccupation)
            )),
        }
        assert_eq!(output.symbols().value(), symbols.value());
        assert_eq!(output.symbols().bound(), symbols.bound());
        assert_eq!(output.symbols().error(), symbols.error());
        assert_eq!(output.specialization_key(), key);
        assert!(std::ptr::eq(output.inventory(), &raw const s.inventory));
        drop(tiles);
        drop(foreign_tiles);
        // Both sets must be dropped before the shared heap can become empty.
        let mut layouts = members.detach();
        if let Some(foreign) = foreign_members {
            layouts.extend(foreign.detach());
        }
        for layout in layouts {
            super::fixture::retire_layout(&mut s.heap, &mut s.account, layout)?;
        }
        super::fixture::empty(&mut s.heap, &mut s.account)?;
    }
    Ok(())
}

/// Public compile-fail twins cover the same surfaces; Debug adds no reconstructible authority.
/// ´test:unit:lc-origin-no-symbol-escape´
#[test]
fn lc_origin_no_symbol_escape() -> TestResult {
    super::execution::lifetime::surface::<f32>()?;
    super::execution::lifetime::surface::<f64>()?;
    debug_surface::<f32>()?;
    debug_surface::<f64>()
}
fn debug_surface<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let prepared = output
        .prepare(
            &mut s.heap,
            &members.references(),
            &s.registry,
            &mut s.account,
        )
        .map_err(|failure| failure.into_parts().1)?;
    assert_eq!(format!("{prepared:?}"), "LcOmegaPrepared { .. }");
    let selected = super::demand(&mut s.heap, &s.axes, &[], &mut s.account)?;
    let mut pending = prepared
        .admit(&mut s.heap, selected, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    assert_eq!(
        format!("{pending:?}"),
        "LcApplicationPending { pending: true, .. }"
    );
    assert!(
        s.heap
            .acquire_pin::<P>(pending.ticket(), &mut s.account)
            .is_err()
    );
    assert!(matches!(
        s.heap.loan_inventory(pending.ticket(), &mut s.account),
        Err(LoanError::WrongLoan)
    ));
    pending.cancel(&mut s.heap, &mut s.account)?;
    assert!(matches!(
        s.heap.loan_inventory(pending.ticket(), &mut s.account),
        Err(LoanError::WrongLoan)
    ));
    drop(pending);
    s.heap.retire_mask(selected, &mut s.account.ledger)?;
    super::fixture::retire_layout(&mut s.heap, &mut s.account, selected.layout())?;
    members.finish(&mut s.heap, &mut s.account)
}

/// Native reservation visits and capture cleanup are pinned independently on used ledgers.
/// ´test:unit:lc-origin-phase-work-boundaries´
#[test]
fn lc_origin_phase_work_boundaries() -> TestResult {
    super::publication::reader_work::<f32>()?;
    super::publication::reader_work::<f64>()?;
    super::publication::work::<f32>()?;
    super::publication::work::<f64>()?;
    super::execution::work::<f32>()?;
    super::execution::work::<f64>()?;
    super::pending::state_work::<f32>()?;
    super::pending::state_work::<f64>()?;
    super::pending::planning::<f32>()?;
    super::pending::planning::<f64>()?;
    super::pending::refusals::<f32>()?;
    super::pending::refusals::<f64>()?;
    capture_work::<f32>()?;
    capture_work::<f64>()?;
    combined_work::<f32>()?;
    combined_work::<f64>()?;
    super::pending::work::<f32>()?;
    super::pending::work::<f64>()
}
fn capture_work<P: Scalar + 'static>() -> TestResult {
    use crate::loan::lc_shape::output::bridge::{borrowed::prepare_borrowed, capture};
    for short in [false, true] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            1,
            0,
        )?;
        let tiles = members.references();
        let (candidate, plan) =
            prepare_borrowed(&output, &s.heap, &tiles, &s.registry, None, &mut s.account)?;
        // Publication leaves host7 and mapped8; each mapped admission peaks at prior pairs+7+17.
        let debit_slots = 4 * (7 + 8) + 7 + 17;
        assert_eq!(s.heap.loans.debits.len(), debit_slots);
        // Host, demand and mapped layout per role; ten persistent masks and one reused demand slot.
        assert_eq!(s.heap.masks.layouts.planned_len(0)?, 15);
        assert_eq!(s.heap.masks.masks.planned_len(0)?, 11);
        assert_eq!(s.heap.storage.scan_work(), 0);
        assert_eq!(s.heap.executions.scan_work(), 0);
        let planning = 3 * debit_slots + 15 + 11;
        // Nine publication vacancies admit both closure roles without replacement.
        let growth = 0;
        let bulk = 14 * 5 + 18 + 2 * growth;
        let exact = (3 + planning + bulk) as u64;
        let remaining = exact - u64::from(short);
        let spend = s.account.limits().work - s.account.usage().work - remaining;
        s.account.ledger.work(spend, crate::loan::AT)?;
        let before = s.account.usage().work;
        let injection = short.then(|| crate::loan::fault::Injection::after(0));
        let result = capture::join(&mut s.heap, candidate, plan, 0, &mut s.account);
        if short {
            assert!(matches!(
                result,
                Err(crate::LcShapeError::Loan(LoanError::Budget(_)))
            ));
            assert_eq!(s.account.usage().work - before, (3 + planning) as u64);
            assert_eq!(crate::loan::fault::Injection::attempts(), 0);
            assert_eq!(s.heap.loans.debits.len(), debit_slots);
        } else {
            let joined = result?;
            assert_eq!(s.account.usage().work - before, exact);
            drop(joined);
        }
        drop(injection);
    }
    Ok(())
}

/// Retained ownership and simultaneous replacement peaks admit exactly and refuse one below.
/// ´test:unit:lc-origin-retained-staged-boundaries´
#[test]
fn lc_origin_retained_staged_boundaries() -> TestResult {
    super::publication::uses::<f32>()?;
    super::publication::uses::<f64>()?;
    super::execution::lifetime::heap_first::<f32>()?;
    super::execution::lifetime::heap_first::<f64>()?;
    super::pending::unwind::<f32>()?;
    super::pending::unwind::<f64>()?;
    super::pending::bytes::<f32>()?;
    super::pending::bytes::<f64>()?;
    super::pending::prefixes::<f32>()?;
    super::pending::prefixes::<f64>()?;
    prepared_bytes::<f32>()?;
    prepared_bytes::<f64>()?;
    scope_boundaries::<f32>()?;
    scope_boundaries::<f64>()
}
fn prepared_bytes<P: Scalar + 'static>() -> TestResult {
    for growing in [false, true] {
        for replay in [false, true] {
            let (retained, peak) = byte_allowances(growing, replay)?;
            for staged in [false, true] {
                for short in [false, true] {
                    let exact = if staged { peak } else { retained };
                    let kind = if staged {
                        BudgetKind::StagedPayloadBytes
                    } else {
                        BudgetKind::OutputBytes
                    };
                    prepared_byte_case::<P>(
                        growing,
                        replay,
                        kind,
                        exact - u64::from(short),
                        exact,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn byte_allowances(growing: bool, replay: bool) -> TestResult<(u64, u64)> {
    let delta = if growing { 2 * 264 } else { 0 };
    let old = if growing { 89 * 264 } else { 0 };
    let phases = [
        280,
        delta + 2 * old + 280,
        delta + 7_800 + 2_144,
        delta + 7_800 + 336,
        delta + 7_800 + 368,
        delta + 7_800 + 872,
        delta + 7_800 + 176,
    ];
    let native = phases.into_iter().max().ok_or("phase inventory")?;
    assert_eq!(native, if growing { 47_800 } else { 9_944 });
    let retained = 424 + oracle::retained(replay) + 128 + 48 + 5 * 1_560 + delta;
    let external = if replay { 10_342 + 24 } else { 0 };
    let peak =
        (424 + oracle::retained(replay) + external + native).max(424 + oracle::staged(replay));
    let expected_retained = if replay { 43_842 } else { 43_866 } + delta;
    assert_eq!(retained, expected_retained);
    let expected_peak = match (growing, replay) {
        (false, false) => 67_392,
        (false, true) => 77_734,
        (true, false) => 83_690,
        (true, true) => 94_032,
    };
    assert_eq!(peak, expected_peak);
    Ok((retained, peak))
}

fn prepared_byte_case<P: Scalar + 'static>(
    growing: bool,
    replay: bool,
    kind: BudgetKind,
    limit: u64,
    exact: u64,
) -> TestResult {
    let slots = if growing { 89 } else { 84 };
    let short = limit < exact;
    let staged = kind == BudgetKind::StagedPayloadBytes;
    let mut limits = Limits::default();
    if !staged {
        limits.output_bytes = limit;
    }
    let mut s = setup::<P>(limits, [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let certificate = if replay {
        let candidate =
            prepare_lc_omega_correspondence(&output, &s.registry, None, &mut s.account)?;
        Some(candidate.certificate(&mut s.account.ledger)?)
    } else {
        None
    };
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let extra = if growing {
        Some(extra_loan(
            &mut s.heap,
            s.inventory.input(0).ok_or("first role")?,
            &s.axes,
            &mut s.account,
        )?)
    } else {
        None
    };
    assert_eq!(s.heap.loans.debits.capacity(), slots);
    let before = s.account.usage().work;
    let injection = short.then(|| crate::loan::fault::Injection::after(0));
    let original_limit = s.account.limits().staged_payload_bytes;
    let caller = if staged { original_limit - limit } else { 0 };
    let identity = s.account.identity;
    let result = s
        .account
        .ledger
        .with_retained_payload(caller, crate::loan::AT, |budget| {
            Ok::<_, LoanError>(output.prepare_on(
                &mut s.heap,
                &members.references(),
                &s.registry,
                certificate.as_ref(),
                identity,
                budget,
            ))
        })?;
    assert_eq!(s.account.limits().staged_payload_bytes, original_limit);
    if short {
        let (_, error) = result
            .expect_err("one byte below bridge maximum")
            .into_parts();
        byte_fault(&error, staged)?;
        assert_eq!(crate::loan::fault::Injection::attempts(), 0);
        assert_eq!(s.heap.loans.debits.len(), slots);
    } else {
        drop(result.map_err(|failure| failure.into_parts().1)?);
        assert_eq!(
            s.heap.loans.debits.capacity(),
            slots + if growing { 2 } else { 0 }
        );
    }
    assert!(s.account.usage().work > before);
    drop(injection);
    if let Some(loan) = extra {
        assert!(matches!(
            s.heap.cancel_loan(loan, &mut s.account),
            crate::CancelOutcome::Cancelled(_)
        ));
    }
    members.finish(&mut s.heap, &mut s.account)
}

fn byte_fault(error: &crate::LcShapeError, staged: bool) -> TestResult {
    use nrz_bindings::readings::algebra::FaultKind;
    let fault = match error {
        crate::LcShapeError::Loan(LoanError::Budget(fault))
        | crate::LcShapeError::Declaration(nrz_bindings::AdmissionError::Algebra(fault)) => fault,
        other => {
            return Err(format!("expected byte refusal, got {other:?}").into());
        }
    };
    let FaultKind::Exhausted {
        budget,
        limit,
        needed,
        ..
    } = fault.kind
    else {
        return Err("expected byte exhaustion".into());
    };
    assert_eq!(
        budget,
        if staged {
            BudgetKind::StagedPayloadBytes
        } else {
            BudgetKind::OutputBytes
        }
    );
    assert_eq!(needed, limit + 1);
    Ok(())
}

fn defensive<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let changed = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    crate::loan::lc_shape::output::bridge::controls::defensive(
        &output,
        &changed,
        &s.heap,
        &members.references(),
        &s.registry,
        &mut s.account,
    )?;
    members.finish(&mut s.heap, &mut s.account)
}

fn combined_work<P: Scalar + 'static>() -> TestResult {
    for growing in [false, true] {
        for replay in [false, true] {
            let (slots, exact, cases) = work_cases(growing, replay);
            for (remaining, completed) in cases {
                let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
                let output = LcOmegaOutput::declare(
                    &s.inventory,
                    request(&s.registry, &s.radian),
                    &mut s.account,
                )?;
                let key = output.specialization_key();
                let symbols = output.symbols().clone();
                let certificate = if replay {
                    let candidate = prepare_lc_omega_correspondence(
                        &output,
                        &s.registry,
                        None,
                        &mut s.account,
                    )?;
                    Some(candidate.certificate(&mut s.account.ledger)?)
                } else {
                    None
                };
                let members = super::super::inventory::readers::publish(
                    &s.inventory,
                    &s.axes,
                    &mut s.heap,
                    &mut s.account,
                    1,
                    0,
                )?;
                let extra = if growing {
                    Some(extra_loan(
                        &mut s.heap,
                        s.inventory.input(0).ok_or("first role")?,
                        &s.axes,
                        &mut s.account,
                    )?)
                } else {
                    None
                };
                let spend = s.account.limits().work - s.account.usage().work - remaining;
                s.account.ledger.work(spend, crate::loan::AT)?;
                let before = s.account.usage().work;
                let refused = remaining != exact;
                let injection = refused.then(|| crate::loan::fault::Injection::after(0));
                let result = if let Some(certificate) = &certificate {
                    output.prepare_replay(
                        &mut s.heap,
                        &members.references(),
                        &s.registry,
                        certificate,
                        &mut s.account,
                    )
                } else {
                    output.prepare(
                        &mut s.heap,
                        &members.references(),
                        &s.registry,
                        &mut s.account,
                    )
                };
                if refused {
                    let (output, error) =
                        result.expect_err("bounded consuming prefix").into_parts();
                    let work_refusal = matches!(
                        error,
                        crate::LcShapeError::Loan(LoanError::Budget(_))
                            | crate::LcShapeError::Declaration(
                                nrz_bindings::AdmissionError::Algebra(_)
                            )
                    );
                    assert!(work_refusal, "{error:?}");
                    assert_eq!(output.specialization_key(), key);
                    assert_eq!(output.symbols().value(), symbols.value());
                    assert_eq!(output.symbols().bound(), symbols.bound());
                    assert_eq!(output.symbols().error(), symbols.error());
                    assert!(std::ptr::eq(output.inventory(), &raw const s.inventory));
                    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
                    assert_eq!(
                        s.heap.loans.debits.len(),
                        usize::try_from(slots).map_err(|_| "debit slots exceed usize")?
                    );
                } else {
                    drop(result.map_err(|failure| failure.into_parts().1)?);
                }
                assert_eq!(s.account.usage().work - before, completed);
                drop(injection);
                if let Some(loan) = extra {
                    let before = s.account.usage().work;
                    assert!(matches!(
                        s.heap.cancel_loan(loan, &mut s.account),
                        crate::CancelOutcome::Cancelled(_)
                    ));
                    assert_eq!(s.account.usage().work, before);
                }
            }
        }
    }
    Ok(())
}

fn work_cases(growing: bool, replay: bool) -> (u64, u64, [(u64, u64); 9]) {
    let binding = oracle::work(replay);
    // Current host18 plus mapped current_record22 per original, separate from final reader3.
    let current = 5 * (18 + 22);
    let slots = if growing { 89 } else { 84 };
    let resource = 2 * slots + if growing { 16 + 11 } else { 15 + 11 };
    let planning = slots + resource;
    let join = planning + 33 * 5 + 31 + current + 2 * u64::from(growing);
    assert_eq!(join, if growing { 692 } else { 674 });
    let exact = 1_017 + binding + join;
    assert_eq!(
        exact,
        match (growing, replay) {
            (false, false) => 337_981,
            (false, true) => 358_773,
            (true, false) => 337_999,
            (true, true) => 358_791,
        }
    );
    let constructed = 2 + 1_017 + binding;
    let native = 1_017 + binding + 305;
    // Ceiling and completed prefix: cheap prepayment, token, CAS, sealed auth,
    // native agreement prepayment, debit scan, resource scan, capture bulk, success.
    let cases = [
        (1, 0),
        (2 + 1_017 - 1, 2 + 1_017 - 2),
        (constructed - 1, constructed - 2),
        (constructed + 2, constructed + 2),
        (constructed + 43 + 44, constructed + 43),
        (native + 3 + slots - 1, native + 3),
        (native + 3 + slots + resource - 1, native + 3 + slots),
        (exact - 1, exact - 88 - 2 * u64::from(growing)),
        (exact, exact),
    ];
    (slots, exact, cases)
}

fn extra_loan<P: Scalar + 'static>(
    heap: &mut Heap,
    schema: &crate::LcInputShape<P>,
    axes: &crate::LcShapeAxes,
    account: &mut ExecutionAccount,
) -> TestResult<crate::OwnedLoan<P>> {
    let declaration = schema.source(axes, account)?;
    let loan = super::fixture::granted(heap.request_loan(declaration, account))?;
    // The final mapped demand was retired: the host mask reuses its vacant slot.
    assert_eq!(heap.masks.layouts.planned_len(0)?, 16);
    assert_eq!(heap.masks.masks.planned_len(0)?, 11);
    Ok(loan)
}

fn portable_refusal<P: Scalar + 'static>() -> TestResult {
    use nrz_bindings::readings::algebra::enclosure::lc_root::Certificate;
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let key = output.specialization_key();
    let original = prepare_lc_omega_correspondence(&output, &s.registry, None, &mut s.account)?;
    let certificate = original.certificate(&mut s.account.ledger)?;
    let mut bytes = certificate.to_bytes(&mut s.account.ledger)?;
    // Ordinary enclosure category magic is not an LC registered-family transcript.
    bytes[..8].copy_from_slice(b"NRZENC01");
    let wrong = Certificate::from_bytes(&bytes, &mut s.account.ledger)?;
    drop(original);
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let before = s.account.usage().work;
    let injection = crate::loan::fault::Injection::after(0);
    let failure = output
        .prepare_replay(
            &mut s.heap,
            &members.references(),
            &s.registry,
            &wrong,
            &mut s.account,
        )
        .expect_err("serialized wrong receipt category");
    let (output, error) = failure.into_parts();
    assert!(matches!(
        error,
        crate::LcShapeError::Declaration(nrz_bindings::AdmissionError::Algebra(_))
    ));
    assert_eq!(output.specialization_key(), key);
    assert!(std::ptr::eq(output.inventory(), &raw const s.inventory));
    assert!(s.account.usage().work > before);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    drop(injection);
    let prepared = output
        .prepare_replay(
            &mut s.heap,
            &members.references(),
            &s.registry,
            &certificate,
            &mut s.account,
        )
        .map_err(|failure| failure.into_parts().1)?;
    drop(prepared);
    members.finish(&mut s.heap, &mut s.account)
}
fn revision_refusal<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let key = output.specialization_key();
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    // Crate-internal exhaustion fault; no public caller can forge a registry revision.
    let revision = s.heap.loans.revision;
    s.heap.loans.revision = u64::MAX;
    let before = s.account.usage().work;
    let injection = crate::loan::fault::Injection::after(0);
    let failure = output
        .prepare(
            &mut s.heap,
            &members.references(),
            &s.registry,
            &mut s.account,
        )
        .expect_err("revision capacity");
    let (output, error) = failure.into_parts();
    assert!(matches!(
        error,
        crate::LcShapeError::Loan(LoanError::RevisionCapacity)
    ));
    assert_eq!(
        s.account.usage().work - before,
        oracle::token_work() + oracle::work(false) + 305 + 3 + 84
    );
    assert_eq!(s.heap.loans.debits.len(), 84);
    assert_eq!(crate::loan::fault::Injection::attempts(), 0);
    assert_eq!(output.specialization_key(), key);
    drop(injection);
    s.heap.loans.revision = revision;
    drop(
        output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|failure| failure.into_parts().1)?,
    );
    members.finish(&mut s.heap, &mut s.account)
}

fn scope_boundaries<P: Scalar + 'static>() -> TestResult {
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    crate::loan::lc_shape::output::bridge::controls::scopes(&output, &s.registry, &mut s.account)
}

fn paused_reconciliation<P: Scalar + 'static>() -> TestResult {
    for reverse in [false, true] {
        let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
        let output = LcOmegaOutput::declare(
            &s.inventory,
            request(&s.registry, &s.radian),
            &mut s.account,
        )?;
        let members = super::super::inventory::readers::publish(
            &s.inventory,
            &s.axes,
            &mut s.heap,
            &mut s.account,
            1,
            0,
        )?;
        let prepared = output
            .prepare(
                &mut s.heap,
                &members.references(),
                &s.registry,
                &mut s.account,
            )
            .map_err(|failure| failure.into_parts().1)?;
        drop(prepared);
        paused_debits(&mut s.heap, &mut s.account, reverse)?;
        members.finish(&mut s.heap, &mut s.account)?;
    }
    Ok(())
}

fn paused_debits(heap: &mut Heap, account: &mut ExecutionAccount, reverse: bool) -> TestResult {
    use crate::loan::registry::AllocationKind;
    let header = heap
        .loans
        .debits
        .iter()
        .position(|slot| {
            slot.as_ref()
                .is_some_and(|d| matches!(d.kind, AllocationKind::LcClosure(_)))
        })
        .ok_or("closure header")?;
    let rows = heap
        .loans
        .debits
        .iter()
        .position(|slot| {
            slot.as_ref()
                .is_some_and(|d| matches!(d.kind, AllocationKind::LcRows(_)))
        })
        .ok_or("closure rows")?;
    // Defensive order permutation after facade drop; neither payload nor token survives.
    let (header, rows) = if reverse {
        heap.loans.debits.swap(header, rows);
        (rows, header)
    } else {
        (header, rows)
    };
    let mut cursor = None;
    let mut saw_rows_removed = false;
    let steps = 2 * (heap.loans.groups.len() + heap.loans.pins.len() + heap.loans.debits.len() + 1);
    for _ in 0..steps {
        let outcome = heap.reconcile_detached(account, 1, cursor);
        let rows_live = heap.loans.debits[rows].is_some();
        let header_live = heap.loans.debits[header].is_some();
        if rows_live {
            assert!(header_live, "rows witness must retain its funded header");
            assert_eq!(
                heap.loans.debits[header]
                    .as_ref()
                    .ok_or("header debit")?
                    .bytes,
                48
            );
        } else if !saw_rows_removed {
            assert!(
                header_live,
                "the one-slot pause after rows release retains the header"
            );
            saw_rows_removed = true;
        }
        if !header_live {
            assert!(saw_rows_removed);
            return Ok(());
        }
        cursor = match outcome {
            crate::ScanOutcome::Pending(next) => Some(next),
            crate::ScanOutcome::Complete => None,
            other => return Err(format!("unexpected paused scan: {other:?}").into()),
        };
    }
    Err("closure debits did not reconcile in two passes".into())
}

/// Owning-layer inventories and the literal independent oracle partition the live union.
/// ´test:unit:lc-omega-graph-retained-overlap´
#[test]
fn lc_omega_graph_retained_overlap() -> TestResult {
    retained_overlap::<f32>()?;
    retained_overlap::<f64>()
}

fn retained_overlap<P: Scalar + 'static>() -> TestResult {
    use crate::loan::lc_shape::output::bridge::controls;
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    controls::retained_overlap(
        &output,
        &s.registry,
        [oracle::retained(false), oracle::retained(true)],
        &mut s.account,
    )?;
    // Independent scenarios get independent original fixtures and accounts. Each
    // failed prefix and its retry stay on one cumulative ledger with unchanged limits.
    for replay in [false, true] {
        for fail_after in [None, Some(0), Some(1), Some(2)] {
            retained_native::<P>(replay, fail_after)?;
        }
    }
    published_overlap::<P>()?;
    // Existing independent native cleanup/transaction witnesses preserve every pin.
    allocation_refusals::<P>()?;
    // Exact/one-below original/helper handoffs, including the replay input.
    scope_boundaries::<P>()
}

fn retained_native<P: Scalar + 'static>(replay: bool, fail_after: Option<usize>) -> TestResult {
    use crate::loan::lc_shape::output::bridge::controls;
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        1,
        0,
    )?;
    let tiles = members.references();
    controls::retained_transfer(
        &output,
        &mut s.heap,
        &tiles,
        &s.registry,
        (replay, oracle::retained(replay), fail_after),
        &mut s.account,
    )?;
    drop(tiles);
    members.finish(&mut s.heap, &mut s.account)
}

fn published_overlap<P: Scalar + 'static>() -> TestResult {
    use crate::loan::lc_shape::output::bridge::{
        controls, prepare_lc_omega_correspondence,
        retained_overlap::{Candidate, Observation},
    };
    let mut s = setup::<P>(Limits::default(), [2, 3, 2])?;
    let output = LcOmegaOutput::declare(
        &s.inventory,
        request(&s.registry, &s.radian),
        &mut s.account,
    )?;
    let members = super::super::inventory::readers::publish(
        &s.inventory,
        &s.axes,
        &mut s.heap,
        &mut s.account,
        2,
        1,
    )?;
    let source = prepare_lc_omega_correspondence(&output, &s.registry, None, &mut s.account)?;
    let construction = Candidate::new(&source)?;
    assert_eq!(construction.captured.logical_allowance, Some(416));
    assert_eq!(construction.initial_slot, 8);
    assert_eq!(
        construction.correspondence.logical_allowance,
        Some(oracle::retained(false))
    );
    assert_eq!(
        construction.logical_allowance,
        424 + oracle::retained(false)
    );
    let certificate = source.certificate(&mut s.account.ledger)?;
    let certificate_report = Observation::certificate(&certificate);
    assert_eq!(certificate_report.observed_residence, Some(10_366));
    assert_eq!(
        construction.logical_allowance + certificate.retained_bytes(),
        46_256
    );
    drop(source);
    let prepared = controls::retained_timeline_prepare(
        output,
        &mut s.heap,
        &members.references(),
        &s.registry,
        (&certificate, oracle::retained(true)),
        &mut s.account,
    )?;
    let witness = std::rc::Rc::downgrade(&prepared.correspondence);
    check_published_overlap(&witness, &certificate, &mut s.account)?;
    let mask = super::demand(&mut s.heap, &s.axes, &[0, 3, 11], &mut s.account)?;
    let pending = prepared
        .admit(&mut s.heap, mask, &mut s.account)
        .map_err(|f| f.into_parts().1)?;
    check_published_overlap(&witness, &certificate, &mut s.account)?;
    let evaluated = pending
        .evaluate(&mut s.account)
        .map_err(super::execution::failure)?;
    check_published_overlap(&witness, &certificate, &mut s.account)?;
    let result = evaluated
        .publish(&mut s.heap, &mut s.account)
        .map_err(|f| f.error)?;
    check_published_overlap(&witness, &certificate, &mut s.account)?;
    let owner = witness
        .upgrade()
        .ok_or("same correspondence survives publication")?;
    let before = s.account.usage();
    let actual = crate::loan::lc_origin::pending::publication::checks::correspondence(&result);
    let report = Observation::correspondence(actual);
    assert_eq!(report.logical_allowance, Some(oracle::retained(true)));
    let same_store =
        report.same_allocation(&Observation::correspondence(&owner), &mut s.account.ledger)?;
    assert!(same_store);
    assert_eq!(s.account.usage(), before);
    drop(owner);
    result.release(&mut s.account).map_err(|(_, error)| error)?;
    // The heap's published bundle still owns the correspondence until its normal
    // residence scan reconciles the released use; dropping a carrier is not that scan.
    assert_eq!(witness.strong_count(), 1);
    check_published_overlap(&witness, &certificate, &mut s.account)?;
    s.heap.residence(&mut s.account, 100_000)?;
    assert_eq!(witness.strong_count(), 0);
    s.heap.retire_mask(mask, &mut s.account.ledger)?;
    super::fixture::retire_layout(&mut s.heap, &mut s.account, mask.layout())?;
    members.finish(&mut s.heap, &mut s.account)?;
    assert_eq!(witness.strong_count(), 0);
    assert_eq!(certificate_report.observed_residence, Some(10_366));
    let same = certificate_report.same_allocation(
        &Observation::certificate(&certificate),
        &mut s.account.ledger,
    )?;
    assert!(same);
    drop(certificate);
    Ok(())
}

fn check_published_overlap(
    witness: &std::rc::Weak<nrz_bindings::LcOmegaCorrespondence>,
    certificate: &nrz_bindings::readings::algebra::enclosure::lc_root::Certificate,
    account: &mut crate::ExecutionAccount,
) -> TestResult {
    use crate::loan::lc_shape::output::bridge::retained_overlap::Observation;
    let owner = witness
        .upgrade()
        .ok_or("replay correspondence remains live")?;
    let shared = std::rc::Rc::clone(&owner);
    let report = Observation::correspondence(&owner);
    assert_eq!(report.logical_allowance, Some(oracle::retained(true)));
    assert_eq!(
        416 + owner.retained_bytes() + certificate.retained_bytes(),
        46_224
    );
    let same =
        report.same_allocation(&Observation::correspondence(&shared), &mut account.ledger)?;
    assert!(same);
    let same_certificate = Observation::certificate(certificate)
        .same_allocation(&Observation::certificate(certificate), &mut account.ledger)?;
    assert!(same_certificate);
    Ok(())
}
