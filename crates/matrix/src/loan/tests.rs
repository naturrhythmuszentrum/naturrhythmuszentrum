// SPDX-License-Identifier: AGPL-3.0-only
//! Independent logical coverage and explicit lifecycle/resource boundary controls.

mod affine;
mod boundaries;
mod invariants;
mod masked_reader;
mod products;
mod recipes;
mod review;
mod solves;
mod statistics;
mod transport;

use super::{
    AbsenceReason, CancelOutcome, ExecutionAccount, HandbackOutcome, HostChannel,
    HostTileDeclaration, LoanError, LoanRequest, OwnedLoan, ProducerOutcome, Residence,
    ScanOutcome,
};
use crate::{Coverage, Extent, Heap, HeapLimits};
use nrz_bindings::{BindingSymbols, Budget, Descriptor, Limits};
use nrz_scalar::Scalar;

/// Test failures retain the typed public cause without reachable production panics.
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
/// Build independent bindings before metering only the native adapter and loan operation.
fn fixture<P: Scalar + 'static>(
    cells: usize,
    components: usize,
    companions: [bool; 2],
    limits: Limits,
) -> TestResult<(Heap, ExecutionAccount, HostTileDeclaration<P>)> {
    let heap = Heap::new(HeapLimits::default())?;
    let mut declarations = Budget::new(Limits::default())?;
    let symbols = BindingSymbols::declare(
        &Descriptor::ratio(),
        [u32::try_from(cells)?, u32::try_from(components)?],
        companions,
        &mut declarations,
    )?;
    let axis = heap.create_cell_axis(cells, &mut declarations)?;
    let mut budget = Budget::new(limits)?;
    budget.work(3, super::AT)?;
    let mut account = ExecutionAccount::new(budget)?;
    let declaration =
        HostTileDeclaration::check(symbols, axis, Coverage::RepresentedInputs, &mut account)?;
    Ok((heap, account, declaration))
}
/// Extract only a granted capability, preserving an informative typed refusal.
fn granted<P: Scalar>(request: LoanRequest<P>) -> TestResult<OwnedLoan<P>> {
    match request {
        LoanRequest::Granted(loan) => Ok(loan),
        LoanRequest::Refused(error) => Err(error.into()),
    }
}
/// Complete bounded scans and independently check all managed numerical debits vanish.
fn empty(heap: &mut Heap, account: &mut ExecutionAccount) -> TestResult {
    let residence = heap.residence(account, 100_000)?;
    assert!(
        matches!(residence, Residence::Empty { .. }),
        "{residence:?}"
    );
    assert_eq!(heap.loans.elements()?, [0, 0]);
    assert!(heap.loans.debits.iter().all(Option::is_none));
    Ok(())
}
/// Logical sets independently predict cell-local closure across both native formats.
fn mask_oracle<P: Scalar + 'static>() -> TestResult {
    for cells in [0, 1, 63, 64, 65, 128] {
        for mode in 0..4 {
            for companions in [[false, false], [true, false], [true, true]] {
                let (mut heap, mut account, declaration) =
                    fixture::<P>(cells, 2, companions, Limits::default())?;
                let mut loan = granted(heap.request_loan(declaration, &mut account))?;
                let mut expected = std::collections::BTreeSet::new();
                for cell in 0..cells {
                    let selected = match mode {
                        0 => false,
                        1 => cell % 7 == 1,
                        2 => cell % 3 != 0,
                        _ => true,
                    };
                    // Invalid bytes in omitted cells prove validation cannot scan every backing scalar.
                    if !selected {
                        loan.write_component(
                            HostChannel::Center,
                            cell,
                            0,
                            P::parse_nearest("NaN").ok_or("NaN parser")?,
                            &mut account,
                        )?;
                        continue;
                    }
                    let tag = P::from_u8(u8::try_from(cell)?);
                    loan.write_cell(HostChannel::Center, cell, &[tag, P::ONE], &mut account)?;
                    if companions[0] {
                        loan.write_cell(HostChannel::Total, cell, &[P::ONE, P::ONE], &mut account)?;
                    }
                    if companions[1] {
                        loan.write_cell(
                            HostChannel::Error,
                            cell,
                            &[P::ZERO, P::ZERO],
                            &mut account,
                        )?;
                    }
                    expected.insert(cell);
                }
                let (tile, receipt) = match heap.handback(loan, &mut account) {
                    HandbackOutcome::Published(tile, receipt) => (tile, receipt),
                    other => return Err(format!("unexpected handback: {other:?}").into()),
                };
                let mask = receipt
                    .filled(HostChannel::Center)
                    .ok_or("center attachment")?;
                let view = heap.mask(mask.native(), mask.native().layout())?;
                for cell in 0..cells {
                    assert_eq!(view.contains(cell)?, expected.contains(&cell));
                }
                if cells % 64 != 0 {
                    assert_eq!(view.words().last().copied().unwrap_or(0) >> (cells % 64), 0);
                }
                let reader = tile.reader(&heap, &mut account)?;
                for cell in 0..cells {
                    let result = reader.read_component(HostChannel::Center, cell, 0, &mut account);
                    if expected.contains(&cell) {
                        assert_eq!(result?.to_f64(), f64::from(u32::try_from(cell)?));
                    } else {
                        assert!(matches!(result, Err(LoanError::CoverageMismatch)));
                    }
                }
                drop(tile);
                empty(&mut heap, &mut account)?;
            }
        }
    }
    Ok(())
}
/// Empty, sparse, partial and full filled masks agree with independent logical sets.
/// ´test:unit:loan-independent-mask-and-companion-oracles´
#[test]
fn loan_independent_mask_and_companion_oracles() -> TestResult {
    mask_oracle::<f32>()?;
    mask_oracle::<f64>()
}

/// A complete value needs all declared companions; repeated writes and clearing remain tracked.
/// ´test:unit:loan-component-intersection-clear-and-orphans´
#[test]
fn loan_component_intersection_clear_and_orphans() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(4, 2, [true, true], Limits::default())?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    for cell in 0..4 {
        loan.write_cell(HostChannel::Center, cell, &[2.0, 3.0], &mut account)?;
        loan.write_cell(HostChannel::Total, cell, &[1.0, 1.0], &mut account)?;
        if cell != 1 {
            loan.write_cell(HostChannel::Error, cell, &[0.0, 0.0], &mut account)?;
        }
    }
    loan.clear_cell(HostChannel::Center, 2, &mut account)?;
    loan.write_component(HostChannel::Center, 2, 0, 8.0, &mut account)?;
    loan.write_component(HostChannel::Center, 0, 1, 9.0, &mut account)?;
    loan.clear_cell(HostChannel::Total, 3, &mut account)?;
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    for channel in HostChannel::ALL {
        let filled = receipt
            .filled(channel)
            .ok_or("missing attachment")?
            .native();
        assert_eq!(heap.mask(filled, filled.layout())?.words(), &[1]);
    }
    assert_eq!(
        tile.reader(&heap, &mut account)?.read_component(
            HostChannel::Center,
            0,
            1,
            &mut account
        )?,
        9.0
    );
    drop(tile);
    empty(&mut heap, &mut account)
}

/// Invalid initialized payload is repairable and cancellation never reads malformed bytes.
fn repair<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) = fixture::<P>(1, 1, [true, true], Limits::default())?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    let ticket = loan.ticket();
    loan.write_cell(HostChannel::Center, 0, &[P::ONE], &mut account)?;
    loan.write_cell(HostChannel::Total, 0, &[P::ZERO], &mut account)?;
    loan.write_cell(HostChannel::Error, 0, &[P::ONE], &mut account)?;
    let mut loan = match heap.handback(loan, &mut account) {
        HandbackOutcome::Repair(
            LoanError::InvalidPayload {
                channel: HostChannel::Error,
                ..
            },
            loan,
        ) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(loan.ticket(), ticket);
    loan.write_cell(HostChannel::Total, 0, &[P::ONE], &mut account)?;
    let tile = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    assert!(matches!(
        heap.probe_loan(ticket, &mut account),
        Err(LoanError::DuplicateReturn)
    ));
    drop(tile);
    empty(&mut heap, &mut account)?;
    assert!(matches!(
        heap.probe_loan(ticket, &mut account),
        Err(LoanError::StaleOccupation)
    ));
    Ok(())
}
/// Error-over-total validation preserves identity through repair and complete publication.
/// ´test:unit:loan-invalid-payload-repair-then-success´
#[test]
fn loan_invalid_payload_repair_then_success() -> TestResult {
    repair::<f32>()?;
    repair::<f64>()
}

/// Foreign heaps and private nonce corruption never mutate the rightful pending owner.
/// ´test:unit:loan-foreign-nonce-stale-and-double-authentication´
#[test]
fn loan_foreign_nonce_stale_and_double_authentication() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(1, 1, [false, false], Limits::default())?;
    let loan = granted(heap.request_loan(declaration, &mut account))?;
    let original = loan.ticket();
    let mut foreign = Heap::new(HeapLimits::default())?;
    let mut loan = match foreign.handback(loan, &mut account) {
        HandbackOutcome::ReturnRejected(LoanError::ForeignOwner, loan) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(heap.loans.elements()?, [0, 1]);
    loan.core.identity.nonce += 1;
    let mut loan = match heap.handback(loan, &mut account) {
        HandbackOutcome::ReturnRejected(LoanError::WrongLoan, loan) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    loan.core.identity = original.identity;
    heap.probe_loan(original, &mut account)?;
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    assert!(matches!(
        heap.probe_loan(original, &mut account),
        Err(LoanError::DuplicateReturn)
    ));
    empty(&mut heap, &mut account)
}

/// Pin duplication has independent charged lifetime; native metadata cannot retire through a pin.
/// ´test:unit:loan-pins-protect-native-owners-and-last-use´
#[test]
fn loan_pins_protect_native_owners_and_last_use() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(1, 1, [false, false], Limits::default())?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    loan.write_cell(HostChannel::Center, 0, &[7.0], &mut account)?;
    let (tile, receipt) = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, receipt) => (tile, receipt),
        other => return Err(format!("{other:?}").into()),
    };
    let before = heap.loans.elements()?;
    let usage = account.usage().work;
    let pin = heap.retain_pin(tile.pin(), &mut account)?;
    assert_eq!(account.usage().work - usage, 54);
    assert_eq!(heap.loans.elements()?, before);
    let native = receipt
        .filled(HostChannel::Center)
        .ok_or("filled")?
        .native();
    assert!(matches!(
        heap.retire_mask(native, &mut account.ledger),
        Err(crate::MaskError::Heap(crate::HeapError::PinnedUses))
    ));
    assert!(matches!(
        heap.retire_cell_layout(native.layout(), &mut account.ledger),
        Err(crate::MaskError::Heap(crate::HeapError::PinnedUses))
    ));
    drop(tile);
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        Residence::Pending { .. }
    ));
    assert_eq!(
        pin.historical_reader(&mut account)?.read_component(
            HostChannel::Center,
            0,
            0,
            &mut account
        )?,
        7.0
    );
    drop(pin);
    empty(&mut heap, &mut account)?;
    assert!(matches!(
        heap.acquire_pin::<f64>(receipt.ticket(), &mut account),
        Err(LoanError::StaleOccupation)
    ));
    Ok(())
}

/// Drop conservatively retains the debit until reconciliation and requires terminal delivery.
/// ´test:unit:loan-abandonment-reconciliation-and-delivery´
#[test]
fn loan_abandonment_reconciliation_and_delivery() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f32>(2, 1, [false, false], Limits::default())?;
    let loan = granted(heap.request_loan(declaration, &mut account))?;
    let ticket = loan.ticket();
    drop(loan);
    assert_eq!(heap.loans.elements()?, [2, 0]);
    assert!(matches!(
        heap.residence(&mut account, 1000)?,
        Residence::Pending { .. }
    ));
    assert_eq!(heap.loans.elements()?, [0, 0]);
    assert!(
        matches!(heap.take_producer_outcome(ticket, &mut account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == AbsenceReason::Abandoned)
    );
    assert_eq!(
        heap.take_producer_outcome(ticket, &mut account)?,
        ProducerOutcome::AlreadyDelivered
    );
    empty(&mut heap, &mut account)
}

/// Work refusal leaves setters unchanged and prepaid cancellation remains available at zero allowance.
/// ´test:unit:loan-literal-work-boundary-and-prepaid-cancellation´
#[test]
fn loan_literal_work_boundary_and_prepaid_cancellation() -> TestResult {
    // One scalar, two flag words, one channel: declaration 1; admission 75; prior usage 3.
    for (limit, succeeds) in [(79, true), (78, false)] {
        let (mut heap, mut account, declaration) = fixture::<f64>(
            1,
            1,
            [false, false],
            Limits {
                work: limit,
                ..Limits::default()
            },
        )?;
        let request = heap.request_loan(declaration, &mut account);
        match request {
            LoanRequest::Granted(mut loan) => {
                assert!(succeeds);
                assert_eq!(account.usage().work, 79);
                assert!(
                    loan.write_cell(HostChannel::Center, 0, &[8.0], &mut account)
                        .is_err()
                );
                assert!(matches!(
                    heap.cancel_loan(loan, &mut account),
                    CancelOutcome::Cancelled(_)
                ));
                assert_eq!(heap.loans.elements()?, [0, 0]);
            }
            LoanRequest::Refused(LoanError::Budget(_)) => assert!(!succeeds),
            other @ LoanRequest::Refused(_) => return Err(format!("{other:?}").into()),
        }
    }
    Ok(())
}

/// Reserved terminal revision headroom survives every intervening ordinary mutation.
/// ´test:unit:loan-terminal-revision-headroom´
#[test]
fn loan_terminal_revision_headroom() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(0, 1, [false, false], Limits::default())?;
    heap.loans.revision = u64::MAX - 2;
    let loan = granted(heap.request_loan(declaration, &mut account))?;
    assert_eq!(heap.loans.revision, u64::MAX - 1);
    assert!(matches!(
        heap.loans.next_revision(),
        Err(LoanError::RevisionCapacity)
    ));
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    assert_eq!(heap.loans.revision, u64::MAX);
    assert_eq!(heap.loans.elements()?, [0, 0]);
    let (mut heap, mut account, declaration) =
        fixture::<f64>(0, 1, [false, false], Limits::default())?;
    heap.loans.revision = u64::MAX - 1;
    assert!(matches!(
        heap.request_loan(declaration, &mut account),
        LoanRequest::Refused(LoanError::RevisionCapacity)
    ));
    Ok(())
}

/// The complete per-precision element sum refuses before any legacy output allocation.
fn combined<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(6, 1, [false, false], Limits::default())?;
    heap.limits = HeapLimits::new(4096, 10, 4096);
    let loan = granted(heap.request_loan(declaration, &mut account))?;
    heap.import_represented(Extent::new(3, 1)?, &Descriptor::ratio(), &[P::ZERO; 3])?;
    let before = heap.storage.elements();
    assert!(matches!(
        heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[P::ONE; 2]),
        Err(crate::HeapError::Capacity)
    ));
    assert_eq!(heap.storage.elements(), before);
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    heap.import_represented(Extent::new(2, 1)?, &Descriptor::ratio(), &[P::ONE; 2])?;
    Ok(())
}
/// Managed six plus legacy three plus requested two cannot fit a ten-element precision ceiling.
/// ´test:unit:loan-managed-legacy-output-common-capacity´
#[test]
fn loan_managed_legacy_output_common_capacity() -> TestResult {
    combined::<f32>()?;
    combined::<f64>()
}

/// A stopped scan retains its next unvisited slot and never reports empty residence.
/// ´test:unit:loan-resumable-scans-and-work-exhaustion´
#[test]
fn loan_resumable_scans_and_work_exhaustion() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(1, 1, [false, false], Limits::default())?;
    let loan = granted(heap.request_loan(declaration, &mut account))?;
    let ticket = loan.ticket();
    drop(loan);
    let mut scanner = ExecutionAccount::new(Budget::new(Limits {
        work: 0,
        ..Limits::default()
    })?)?;
    let cursor = match heap.reconcile_detached(&mut scanner, 1, None) {
        ScanOutcome::WorkExhausted(cursor) => cursor,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(heap.loans.elements()?, [0, 1]);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 100, Some(cursor)),
        ScanOutcome::Complete
    ));
    heap.take_producer_outcome(ticket, &mut account)?;
    empty(&mut heap, &mut account)
}

/// All three legacy binding entry points see the same managed reservation before output zeros.
fn binding_capacity<P: Scalar + 'static>() -> TestResult {
    for path in 0..3 {
        let (mut heap, mut account, declaration) =
            fixture::<P>(6, 1, [false, false], Limits::default())?;
        heap.limits = HeapLimits::new(4096, 10, 4096);
        let loan = granted(heap.request_loan(declaration, &mut account))?;
        let input =
            heap.import_represented(Extent::new(3, 1)?, &Descriptor::ratio(), &[P::ONE; 3])?;
        let selection = heap.alias(
            input.value(),
            &crate::Layout::strided(Extent::new(2, 1)?, 3, 0, 1, 2)?,
        )?;
        let binding = crate::Binding::new(
            crate::binding_testing::operation(2, 1)?,
            vec![selection],
            crate::ExecutionLimits::default(),
        );
        let policy = Budget::new(Limits::default())?;
        let result = match path {
            0 => binding.validate(&mut heap),
            1 => binding.validate_with_resource_budget(&mut heap, &policy),
            _ => binding.validate_with_account(&mut heap, &mut account),
        };
        assert!(matches!(
            result,
            Err(crate::OperationError::Heap(crate::HeapError::Capacity))
        ));
        drop(result);
        assert_eq!(
            heap.storage.elements()[crate::storage::Format::of::<P>().index()],
            3
        );
        assert!(matches!(
            heap.cancel_loan(loan, &mut account),
            CancelOutcome::Cancelled(_)
        ));
    }
    Ok(())
}
/// Default, immutable-policy and original-account validation close the same counterexample.
/// ´test:unit:loan-all-legacy-binding-paths-share-capacity´
#[test]
fn loan_all_legacy_binding_paths_share_capacity() -> TestResult {
    binding_capacity::<f32>()?;
    binding_capacity::<f64>()
}

/// A detached loan does not retain the exclusive heap execution lease.
fn coexistence<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(2, 1, [false, false], Limits::default())?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    let original = loan.ticket();
    let input = heap.import_represented(Extent::new(1, 1)?, &Descriptor::ratio(), &[P::ONE])?;
    let bound = crate::Binding::new(
        crate::binding_testing::operation(1, 1)?,
        vec![input.value()],
        crate::ExecutionLimits::default(),
    )
    .validate_with_account(&mut heap, &mut account)?;
    assert_eq!(bound.resource_permit.account, Some(account.identity));
    let candidate = bound.evaluate(bound.normalized_inputs()?)?;
    let outcome = bound.publish(candidate)?;
    assert_eq!(outcome.output().len(), 1);
    heap.probe_loan(original, &mut account)?;
    loan.write_cell(HostChannel::Center, 1, &[P::ONE], &mut account)?;
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    Ok(())
}
/// Both formats complete unrelated ordinary exclusive execution while a managed loan is out.
/// ´test:unit:loan-coexists-with-unrelated-exclusive-binding´
#[test]
fn loan_coexists_with_unrelated_exclusive_binding() -> TestResult {
    coexistence::<f32>()?;
    coexistence::<f64>()
}

/// Dropping a repair wrapper on unwind expires the same witness after all payload fields.
/// ´test:unit:loan-repair-drop-and-unwind-preserve-abandonment-identity´
#[test]
fn loan_repair_drop_and_unwind_preserve_abandonment_identity() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(1, 1, [false, false], Limits::default())?;
    let mut loan = granted(heap.request_loan(declaration, &mut account))?;
    let ticket = loan.ticket();
    loan.write_cell(HostChannel::Center, 0, &[f64::NAN], &mut account)?;
    let weak = std::rc::Rc::downgrade(&loan.core.witness);
    let repair = heap.handback(loan, &mut account);
    assert!(matches!(
        &repair,
        HandbackOutcome::Repair(LoanError::InvalidPayload { .. }, _)
    ));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let owner = repair;
        assert!(matches!(&owner, HandbackOutcome::Repair(_, _)));
        assert_eq!(weak.strong_count(), 1);
        panic!("exercise destruction during unwind");
    }));
    assert!(result.is_err());
    assert_eq!(weak.strong_count(), 0);
    assert_eq!(heap.loans.elements()?, [0, 1]);
    assert!(matches!(
        heap.reconcile_detached(&mut account, 1000, None),
        ScanOutcome::Complete
    ));
    assert!(
        matches!(heap.take_producer_outcome(ticket, &mut account)?, ProducerOutcome::Absent(receipt) if receipt.reason() == AbsenceReason::Abandoned)
    );
    empty(&mut heap, &mut account)
}

/// Every allocation refusal closes only its acquired preparation prefix and retains no producer.
/// ´test:unit:loan-all-admission-allocation-prefixes-clean-up´
#[test]
fn loan_all_admission_allocation_prefixes_clean_up() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<f64>(2, 2, [true, true], Limits::default())?;
    let injection = super::fault::Injection::after(usize::MAX);
    let loan = granted(heap.request_loan(declaration, &mut account))?;
    let allocations = super::fault::Injection::attempts();
    drop(injection);
    assert!(matches!(
        heap.cancel_loan(loan, &mut account),
        CancelOutcome::Cancelled(_)
    ));
    for prefix in 0..allocations {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(2, 2, [true, true], Limits::default())?;
        let injection = super::fault::Injection::after(prefix);
        let result = heap.request_loan(declaration, &mut account);
        assert!(
            matches!(
                result,
                LoanRequest::Refused(
                    LoanError::AllocationRefused | LoanError::Heap(crate::HeapError::Capacity)
                )
            ),
            "prefix {prefix}: {result:?}"
        );
        assert_eq!(super::fault::Injection::attempts(), prefix + 1);
        let [acquired, retained, destroyed] = super::fault::Injection::prefix();
        assert_eq!(acquired, prefix);
        assert_eq!(destroyed + retained, acquired);
        drop(injection);
        assert_eq!(heap.descriptors.slots.iter().count(), 0);
        assert_eq!(heap.masks.layouts.iter().count(), 0);
        assert_eq!(heap.masks.masks.iter().count(), 0);
        assert!(heap.loans.debits.iter().all(Option::is_none));
        assert!(heap.loans.groups.iter().all(Option::is_none));
        assert_eq!(heap.loans.pending, 0);
        assert!(account.usage().work > 3);
    }
    Ok(())
}

/// Valid payload can fail late publication allocation and still repair or cancel at exhausted Work.
/// ´test:unit:loan-late-publication-allocation-repair-and-cancel´
#[test]
fn loan_late_publication_allocation_repair_and_cancel() -> TestResult {
    for cancel in [false, true] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(1, 1, [false, false], Limits::default())?;
        let mut loan = granted(heap.request_loan(declaration, &mut account))?;
        loan.write_cell(HostChannel::Center, 0, &[11.0], &mut account)?;
        let ticket = loan.ticket();
        let weak = std::rc::Rc::downgrade(&loan.core.witness);
        // Debit-table growth and filled-word allocation precede the immutable mask shell.
        let injection = super::fault::Injection::after(2);
        let loan = match heap.handback(loan, &mut account) {
            HandbackOutcome::Repair(LoanError::AllocationRefused, loan) => loan,
            other => return Err(format!("{other:?}").into()),
        };
        assert_eq!(super::fault::Injection::attempts(), 3);
        drop(injection);
        assert_eq!(loan.ticket(), ticket);
        assert_eq!(weak.strong_count(), 1);
        assert_eq!(heap.loans.elements()?, [0, 1]);
        if cancel {
            let remaining = account.limits().work - account.usage().work;
            account.ledger.work(remaining, super::AT)?;
            assert!(matches!(
                heap.cancel_loan(loan, &mut account),
                CancelOutcome::Cancelled(_)
            ));
            assert_eq!(heap.loans.elements()?, [0, 0]);
        } else {
            let tile = match heap.handback(loan, &mut account) {
                HandbackOutcome::Published(tile, _) => tile,
                other => return Err(format!("{other:?}").into()),
            };
            assert_eq!(
                tile.reader(&heap, &mut account)?.read_component(
                    HostChannel::Center,
                    0,
                    0,
                    &mut account
                )?,
                11.0
            );
            drop(tile);
            empty(&mut heap, &mut account)?;
        }
    }
    Ok(())
}
