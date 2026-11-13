// SPDX-License-Identifier: AGPL-3.0-only
//! Physical address oracles are independent of the native layout resolver.

use super::{TestResult, empty, fixture, granted};
use crate::{HandbackOutcome, HostChannel, Layout, LoanError, LoanRequest};
use nrz_bindings::Limits;
use nrz_scalar::Scalar;

/// Independently specified dense and gapped permutations have distinct physical footprints.
fn layout(kind: usize) -> TestResult<Layout> {
    let extent = crate::Extent::new(65, 2)?;
    Ok(match kind {
        0 => Layout::strided(extent, 137, 3, 1, 65)?,
        1 => Layout::strided(extent, 137, 3, 2, 1)?,
        2 => Layout::strided(extent, 137, 132, -2, -1)?,
        3 => Layout::strided(extent, 400, 3, 3, 201)?,
        4 => Layout::strided(extent, 134, 0, 2, 3)?,
        _ => return Err("unknown test layout".into()),
    })
}

/// Tag every selected value while leaving a malformed component in each hole.
fn fill<P: Scalar + 'static>(
    loan: &mut crate::OwnedLoan<P>,
    account: &mut crate::ExecutionAccount,
    mode: usize,
) -> TestResult<Vec<usize>> {
    let mut selected = Vec::new();
    for cell in 0..65 {
        let include = match mode {
            0 => false,
            1 => [1, 7, 64].contains(&cell),
            2 => cell % 3 != 0,
            3 => true,
            _ => return Err("unknown coverage mode".into()),
        };
        if !include {
            loan.write_component(
                HostChannel::Center,
                cell,
                0,
                P::parse_nearest("NaN").ok_or("NaN")?,
                account,
            )?;
            continue;
        }
        let tag = P::from_u8(u8::try_from(cell + 2)?);
        loan.write_cell(HostChannel::Center, cell, &[tag, P::ONE], account)?;
        loan.write_cell(HostChannel::Total, cell, &[P::ONE, P::ONE], account)?;
        loan.write_cell(HostChannel::Error, cell, &[P::ZERO, P::ZERO], account)?;
        selected.push(cell);
    }
    Ok(selected)
}

/// Both formats retain logical companion closure across planar, interleaved and reverse maps.
/// ´test:unit:loan-dense-layout-independent-addresses-and-coverage´
#[test]
fn loan_dense_layout_independent_addresses_and_coverage() -> TestResult {
    coverage::<f32>(0..3)?;
    coverage::<f64>(0..3)
}

/// Explicit address arithmetic, logical set membership and pinned reads are separate oracles.
fn coverage<P: Scalar + 'static>(kinds: std::ops::Range<usize>) -> TestResult {
    for kind in kinds {
        for mode in 0..4 {
            let (mut heap, mut account, declaration) =
                fixture::<P>(65, 2, [true, true], Limits::default())?;
            let mut loan =
                granted(heap.request_loan_with_layout(declaration, layout(kind)?, &mut account))?;
            let selected = fill(&mut loan, &mut account, mode)?;
            check_physical(&loan, &selected, kind)?;
            let tile = match heap.handback(loan, &mut account) {
                HandbackOutcome::Published(tile, _) => tile,
                other => return Err(format!("{other:?}").into()),
            };
            let pin = heap.retain_pin(tile.pin(), &mut account)?;
            drop(tile);
            check_reader(&pin, &heap, &mut account, &selected)?;
            drop(pin);
            empty(&mut heap, &mut account)?;
        }
    }
    Ok(())
}

/// Repair and clear act on original logical components even when physical order is reversed.
/// ´test:unit:loan-dense-layout-repair-and-clear´
#[test]
fn loan_dense_layout_repair_and_clear() -> TestResult {
    repair::<f32>()?;
    repair::<f64>()
}

/// A failed handback preserves the exact ticket; clearing one component does not fill its cell.
fn repair<P: Scalar + 'static>() -> TestResult {
    let (mut heap, mut account, declaration) =
        fixture::<P>(65, 2, [true, true], Limits::default())?;
    let mut loan = granted(heap.request_loan_with_layout(declaration, layout(2)?, &mut account))?;
    fill(&mut loan, &mut account, 1)?;
    loan.write_component(HostChannel::Error, 7, 1, P::from_u8(2), &mut account)?;
    let ticket = loan.ticket();
    let mut loan = match heap.handback(loan, &mut account) {
        HandbackOutcome::Repair(
            LoanError::InvalidPayload {
                channel: HostChannel::Error,
                cell: 7,
                component: 1,
            },
            loan,
        ) => loan,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(loan.ticket(), ticket);
    loan.write_component(HostChannel::Error, 7, 1, P::ZERO, &mut account)?;
    loan.clear_cell(HostChannel::Total, 64, &mut account)?;
    loan.write_component(HostChannel::Total, 64, 0, P::ONE, &mut account)?;
    let tile = match heap.handback(loan, &mut account) {
        HandbackOutcome::Published(tile, _) => tile,
        other => return Err(format!("{other:?}").into()),
    };
    let reader = tile.reader(&heap, &mut account)?;
    assert!(reader.contains(1, &mut account)?);
    assert!(reader.contains(7, &mut account)?);
    assert!(!reader.contains(64, &mut account)?);
    drop(tile);
    empty(&mut heap, &mut account)
}

/// Alias and wrong-extent maps refuse before numerical owners or native occupations exist.
/// ´test:unit:loan-dense-layout-refusals´
#[test]
fn loan_dense_layout_refusals() -> TestResult {
    for map in [
        Layout::strided(crate::Extent::new(65, 2)?, 137, 0, 0, 1)?,
        Layout::strided(crate::Extent::new(65, 2)?, 300, 0, 2, 4)?,
        Layout::contiguous(crate::Extent::new(64, 2)?)?,
    ] {
        let (mut heap, mut account, declaration) =
            fixture::<f64>(65, 2, [true, true], Limits::default())?;
        assert!(matches!(
            heap.request_loan_with_layout(declaration, map, &mut account),
            LoanRequest::Refused(LoanError::LayoutMismatch)
        ));
        assert_eq!(heap.loans.elements()?, [0, 0]);
        assert_eq!(heap.descriptors.slots.iter().count(), 0);
    }
    Ok(())
}

/// The oracle never invokes the production map to predict a physical address.
fn check_physical<P: Scalar + 'static>(
    loan: &crate::OwnedLoan<P>,
    selected: &[usize],
    kind: usize,
) -> TestResult {
    let center = &loan.core.channels[0].as_ref().ok_or("center")?.matrix;
    for &cell in selected {
        for component in 0..2 {
            let address = match kind {
                0 => 3 + cell + 65 * component,
                1 => 3 + 2 * cell + component,
                2 => 132 - 2 * cell - component,
                3 => 3 + 3 * cell + 201 * component,
                4 => 2 * cell + 3 * component,
                _ => return Err("unknown test layout".into()),
            };
            let expected = if component == 0 {
                P::from_u8(u8::try_from(cell + 2)?)
            } else {
                P::ONE
            };
            assert_eq!(center.read::<P>(address)?, expected);
        }
    }
    Ok(())
}

/// Both ordinary and mask-selected reads use the retained physical component permutation.
fn check_reader<P: Scalar + 'static>(
    pin: &crate::BundlePin<P>,
    heap: &crate::Heap,
    account: &mut crate::ExecutionAccount,
    selected: &[usize],
) -> TestResult {
    let reader = pin.historical_reader(account)?;
    let mask = pin.ticket()?.identity.mask;
    let view = reader.masked_view(heap.mask(mask, mask.layout())?, account)?;
    for cell in 0..65 {
        assert_eq!(reader.contains(cell, account)?, selected.contains(&cell));
        if selected.contains(&cell) {
            assert_eq!(
                reader.read_component(HostChannel::Center, cell, 0, account)?,
                P::from_u8(u8::try_from(cell + 2)?)
            );
            assert_eq!(
                reader.read_component(HostChannel::Total, cell, 1, account)?,
                P::ONE
            );
            assert_eq!(
                reader.read_component(HostChannel::Error, cell, 0, account)?,
                P::ZERO
            );
            assert_eq!(
                view.read_component(HostChannel::Center, cell, 0, account)?,
                P::from_u8(u8::try_from(cell + 2)?)
            );
        } else {
            super::super::fault::reset_reads();
            assert!(matches!(
                reader.read_component(HostChannel::Center, cell, 0, account),
                Err(LoanError::CoverageMismatch)
            ));
            assert!(matches!(
                view.read_component(HostChannel::Center, cell, 0, account),
                Err(LoanError::CoverageMismatch)
            ));
            assert_eq!(super::super::fault::reads(), 0);
        }
    }
    Ok(())
}

/// Gapped planes and interwoven coprime strides preserve cells, companions and physical padding.
/// ´test:unit:loan-affine-layout-independent-addresses-and-coverage´
#[test]
fn loan_affine_layout_independent_addresses_and_coverage() -> TestResult {
    coverage::<f32>(3..5)?;
    coverage::<f64>(3..5)
}
