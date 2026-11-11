// SPDX-License-Identifier: AGPL-3.0-only
//! Independent selected sets and exact rational square root images cover nonnegative boxes and extremes.

use super::{TestResult, available, candidate, fixture, oracle, published, ready, requested};
use crate::loan::fault;
use crate::{CellOutcome, DomainReason, Elementwise, HostChannel, KernelError, LoanError};
use nrz_bindings::Limits;

/// Fetch only through the public result reader; endpoints are deliberately unavailable.
pub(super) fn readings(
    reader: &crate::TileReader<'_, f64>,
    cell: usize,
    component: usize,
    account: &mut crate::ExecutionAccount,
) -> TestResult<[f64; 3]> {
    let values = [
        reader.read_component(HostChannel::Center, cell, component, account)?,
        reader.read_component(HostChannel::Total, cell, component, account)?,
        reader.read_component(HostChannel::Error, cell, component, account)?,
    ];
    let before = fault::reads();
    assert!(matches!(
        reader.expm1_endpoints(cell, component, account),
        Err(LoanError::UnsupportedContract)
    ));
    assert_eq!(fault::reads(), before);
    Ok(values)
}
/// Missing declared error gates availability even though only center/total are loaded.
/// ´test:unit:square_root-independent-selected-sets-and-native-visits´
#[test]
fn square_root_independent_selected_sets_and_native_visits() -> TestResult {
    for cells in [0, 1, 63, 64, 65, 128] {
        for components in [1, 3] {
            for channels in 1..=3 {
                for selection in 0..3 {
                    let mut setup = fixture::<f64>(
                        cells,
                        components,
                        channels,
                        selection,
                        true,
                        Limits::default(),
                    )?;
                    let owner = ready(
                        setup
                            .heap
                            .request_pointwise_square_root(setup.request, &mut setup.account),
                    )?;
                    let expected = (0..cells)
                        .filter(|cell| {
                            requested(*cell, selection)
                                && available(*cell, components, channels, true)
                        })
                        .collect::<Vec<_>>();
                    fault::reset_reads();
                    fault::reset_scalars();
                    fault::reset_stores();
                    let injection = fault::Injection::after(0);
                    let owner = candidate(owner, &mut setup.account)?;
                    assert_eq!(fault::Injection::attempts(), 0);
                    assert_eq!(fault::Injection::kernel_allocations(), [0; 2]);
                    drop(injection);
                    assert_eq!(
                        fault::reads(),
                        (if channels == 1 { 1 } else { 2 }) * components * expected.len()
                    );
                    assert_eq!(fault::scalars(), components * expected.len());
                    assert_eq!(fault::stores(), 3 * components * expected.len());
                    let (tile, _) =
                        published(setup.heap.publish_application(owner, &mut setup.account))?;
                    let reader = tile.reader(&setup.heap, &mut setup.account)?;
                    for cell in 0..cells {
                        assert_eq!(
                            reader.contains(cell, &mut setup.account)?,
                            expected.contains(&cell)
                        );
                        let outcome = reader.application_outcome(cell, &mut setup.account)?;
                        if expected.contains(&cell) {
                            assert!(matches!(outcome, CellOutcome::Completed));
                            for component in 0..components {
                                let actual =
                                    readings(&reader, cell, component, &mut setup.account)?;
                                assert_eq!(actual[0], if cell % 2 == 0 { 2.0 } else { 3.0 });
                            }
                        } else {
                            if requested(cell, selection) {
                                assert!(matches!(outcome, CellOutcome::MissingInput));
                            } else {
                                assert!(matches!(outcome, CellOutcome::NotRequested));
                            }
                            let before = fault::reads();
                            assert!(matches!(
                                reader.read_component(
                                    HostChannel::Center,
                                    cell,
                                    0,
                                    &mut setup.account
                                ),
                                Err(LoanError::CoverageMismatch)
                            ));
                            assert_eq!(fault::reads(), before);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
/// Exact rational images cover irrational and exact roots, rounding neighbors and finite extremes.
/// ´test:unit:square_root-published-exact-rational-value-and-enclosure-oracles´
#[test]
fn square_root_published_exact_rational_value_and_enclosure_oracles() -> TestResult {
    let values = [
        (0.0, 0.0),
        (-0.0, 0.0),
        (1.0, 0.0),
        (4.0, 0.0),
        (2.0, 0.25),
        (1.0, 1.0),
        (1.0_f64.next_up(), 0.0),
        (1.0_f64.next_down(), 0.0),
        (4.0_f64.next_up(), 0.0),
        (4.0_f64.next_down(), 0.0),
        (f64::from_bits(1), 0.0),
        (f64::MIN_POSITIVE.next_down(), 0.0),
        (f64::MIN_POSITIVE, 0.0),
        (f64::MIN_POSITIVE.next_up(), 0.0),
        (f64::from_bits(0x7fd0_0000_0000_0000), 0.0),
        (f64::MAX.next_down(), 0.0),
        // Squares of midpoints 1+2^-53 and 1+3*2^-53 lie strictly between these adjacent inputs.
        (f64::from_bits(0x3ff0_0000_0000_0002), 0.0),
        (f64::from_bits(0x3ff0_0000_0000_0003), 0.0),
        (f64::from_bits(0x3ff0_0000_0000_0004), 0.0),
        (f64::from_bits(0x4010_0000_0000_0002), 0.0),
    ];
    let mut setup = fixture::<f64>(values.len(), 1, 2, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |cell, _| values[cell])?;
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_square_root(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for (cell, (input, radius)) in values.into_iter().enumerate() {
        assert!(matches!(
            reader.application_outcome(cell, &mut setup.account)?,
            CellOutcome::Completed
        ));
        let actual = readings(&reader, cell, 0, &mut setup.account)?;
        oracle::check(input, radius, actual)?;
        boundary_pin(input, radius, actual);
    }
    drop(setup.heap);
    let reader = tile.pin().historical_reader(&mut setup.account)?;
    for (cell, (input, radius)) in values.into_iter().enumerate() {
        let actual = readings(&reader, cell, 0, &mut setup.account)?;
        oracle::check(input, radius, actual)?;
        boundary_pin(input, radius, actual);
    }
    Ok(())
}
/// The error role is included in total and has no numerical load or second radius contribution.
#[test]
fn square_root_declared_error_is_not_counted_twice() -> TestResult {
    let mut results = Vec::new();
    for channels in [2, 3] {
        let mut setup = fixture::<f64>(2, 1, channels, 2, false, Limits::default())?;
        let owner = ready(
            setup
                .heap
                .request_pointwise_square_root(setup.request, &mut setup.account),
        )?;
        fault::reset_reads();
        let owner = candidate(owner, &mut setup.account)?;
        assert_eq!(fault::reads(), 4);
        let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
        let reader = tile.reader(&setup.heap, &mut setup.account)?;
        let pair = [
            readings(&reader, 0, 0, &mut setup.account)?,
            readings(&reader, 1, 0, &mut setup.account)?,
        ];
        for (input, values) in [4.0, 9.0].into_iter().zip(pair) {
            oracle::check(input, 0.25, values)?;
        }
        results.push(pair.map(|values| values.map(f64::to_bits)));
    }
    assert_eq!(results[0], results[1]);
    Ok(())
}
/// Root boundaries follow outward arithmetic, without a normal-operand threshold.
/// ´test:unit:square_root-domain-boundaries-first-refusal-and-continued-traversal´
#[test]
fn square_root_domain_boundaries_first_refusal_and_continued_traversal() -> TestResult {
    for (input, radius, reason) in [
        (-1.0, 0.0, DomainReason::NegativeRoot),
        (-f64::from_bits(1), 0.0, DomainReason::NegativeRoot),
        (1.0, 2.0, DomainReason::NegativeRoot),
        (-f64::MAX, f64::MAX, DomainReason::NegativeRoot),
        (f64::MAX, 0.0, DomainReason::UnboundedImage),
        (f64::MAX / 2.0, f64::MAX / 2.0, DomainReason::UnboundedImage),
        (
            f64::from_bits(0x7fe0_0000_0000_0000),
            f64::from_bits(0x7fe0_0000_0000_0000),
            DomainReason::UnboundedImage,
        ),
    ] {
        for failing in [&[2][..], &[0, 2][..], &[1, 2][..]] {
            let mut setup = fixture::<f64>(3, 3, 2, 2, false, Limits::default())?;
            super::super::refusal::replace_source(&mut setup, |cell, component| {
                if cell == 1 && failing.contains(&component) {
                    (input, radius)
                } else {
                    (2.0, 0.0)
                }
            })?;
            let owner = ready(
                setup
                    .heap
                    .request_pointwise_square_root(setup.request, &mut setup.account),
            )?;
            fault::reset_reads();
            fault::reset_scalars();
            fault::reset_stores();
            let owner = candidate(owner, &mut setup.account)?;
            assert_eq!(
                [fault::reads(), fault::scalars(), fault::stores()],
                [18, 9, 27]
            );
            assert!(
                matches!(owner.core.evidence.outcomes[1], CellOutcome::CellRefused { component, reason: KernelError::Domain { function: Elementwise::SquareRoot, entry, reason: actual } } if component == failing[0] && entry == 1 + 3 * failing[0] && actual == reason)
            );
            let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
            let reader = tile.reader(&setup.heap, &mut setup.account)?;
            assert!(!reader.contains(1, &mut setup.account)?);
            for cell in [0, 2] {
                assert!(reader.contains(cell, &mut setup.account)?);
                oracle::check(2.0, 0.0, readings(&reader, cell, 2, &mut setup.account)?)?;
            }
        }
    }
    Ok(())
}
/// An empty completed set is terminal and publishable even when every selected cell refused.
#[test]
fn square_root_all_refused_publication() -> TestResult {
    let mut setup = fixture::<f64>(65, 2, 1, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |cell, _| {
        (if cell % 2 == 0 { -1.0 } else { f64::MAX }, 0.0)
    })?;
    let owner = candidate(
        ready(
            setup
                .heap
                .request_pointwise_square_root(setup.request, &mut setup.account),
        )?,
        &mut setup.account,
    )?;
    assert_eq!(owner.core.filled, [0, 0]);
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    for cell in 0..65 {
        assert!(!reader.contains(cell, &mut setup.account)?);
        assert!(matches!(
            reader.application_outcome(cell, &mut setup.account)?,
            CellOutcome::CellRefused { component: 0, .. }
        ));
    }
    Ok(())
}

/// Independently derived primitive identities distinguish nominal error from whole-box radius.
fn boundary_pin(input: f64, radius: f64, actual: [f64; 3]) {
    let [center, total, error] = actual;
    if input == 0.0 {
        assert_eq!(actual.map(f64::to_bits), [0; 3]);
    } else if input.to_bits() == 1.0_f64.to_bits() {
        assert_eq!(center, 1.0);
        assert_eq!(error.to_bits(), 0);
        if radius == 0.0 {
            // The downward input neighbor, outward root and outward displacement give (1+2^-52)*2^-52.
            assert_eq!(total.to_bits(), 0x3cb0_0000_0000_0001);
        } else {
            assert!(total >= 1.0);
        }
    } else if input.to_bits() == 4.0_f64.to_bits() {
        assert_eq!(center, 2.0);
        assert!(error > 0.0);
        assert!(total > 0.0);
    } else if input.to_bits() == 1 {
        assert_eq!(center.to_bits(), 0x1e60_0000_0000_0000);
    } else if input.to_bits() == 0x7fd0_0000_0000_0000 {
        assert_eq!(center.to_bits(), 0x5fe0_0000_0000_0000);
    }
}
/// A negative domain precedes a finite-range failure, while all six components still run.
#[test]
fn square_root_mixed_refusals_keep_first_and_visit_later_cell() -> TestResult {
    let mut setup = fixture::<f64>(2, 3, 2, 2, false, Limits::default())?;
    super::super::refusal::replace_source(&mut setup, |cell, component| {
        (
            if cell == 0 {
                [-1.0, f64::MAX, 4.0][component]
            } else {
                4.0
            },
            0.0,
        )
    })?;
    let owner = ready(
        setup
            .heap
            .request_pointwise_square_root(setup.request, &mut setup.account),
    )?;
    fault::reset_reads();
    fault::reset_scalars();
    fault::reset_stores();
    let owner = candidate(owner, &mut setup.account)?;
    assert_eq!(
        [fault::reads(), fault::scalars(), fault::stores()],
        [12, 6, 18]
    );
    assert!(matches!(
        owner.core.evidence.outcomes[0],
        CellOutcome::CellRefused {
            component: 0,
            reason: KernelError::Domain {
                function: Elementwise::SquareRoot,
                entry: 0,
                reason: DomainReason::NegativeRoot
            }
        }
    ));
    let (tile, _) = published(setup.heap.publish_application(owner, &mut setup.account))?;
    let reader = tile.reader(&setup.heap, &mut setup.account)?;
    assert!(!reader.contains(0, &mut setup.account)?);
    assert!(reader.contains(1, &mut setup.account)?);
    for component in 0..3 {
        oracle::check(
            4.0,
            0.0,
            readings(&reader, 1, component, &mut setup.account)?,
        )?;
    }
    Ok(())
}
